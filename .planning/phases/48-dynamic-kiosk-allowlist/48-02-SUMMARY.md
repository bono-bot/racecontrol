---
phase: 48-dynamic-kiosk-allowlist
plan: "02"
subsystem: rc-agent/kiosk
tags: [kiosk, allowlist, llm, e2e-test, security]
dependency_graph:
  requires: [48-01]
  provides: [ALLOW-03, ALLOW-04, ALLOW-05]
  affects: [rc-agent, kiosk enforcement, process classification]
tech_stack:
  added: []
  patterns:
    - OnceLock pattern for global server allowlist (matches learned_allowlist)
    - allowlist_poll_loop with MissedTickBehavior::Skip (5-min refresh)
    - tokio::spawn LLM classification that never blocks enforcement loop
    - spawn_blocking returning pending_classifications for async post-processing
key_files:
  created:
    - tests/e2e/api/kiosk-allowlist.sh
  modified:
    - crates/rc-agent/src/kiosk.rs
    - crates/rc-agent/src/main.rs
decisions:
  - server_allowlist is a 4th additive layer (hardcoded > learned > server > temp) — never replaces baseline
  - enforce_process_whitelist_blocking returns pending_classifications so LLM fires from outer async context
  - classify_process defaults to ProcessVerdict::Ask on LLM failure — never auto-kills on uncertainty
  - BLOCK verdict checked first (critical), then ALLOW, then ASK — safe ordering
  - allowlist poll loop first tick fires immediately (MissedTickBehavior::Skip, interval fires on first tick())
  - HTTP base URL derived from WebSocket URL by replacing ws:// and stripping /ws/ suffix
metrics:
  duration_secs: 564
  completed_date: "2026-03-19"
  tasks_completed: 2
  files_modified: 3
---

# Phase 48 Plan 02: Server Allowlist Poll Loop + LLM Classifier Summary

rc-agent dynamically fetches staff-managed allowlist from server every 5 minutes, classifies unknown processes via local Ollama LLM before any kill action, and E2E test validates the full CRUD cycle.

## Tasks Completed

| Task | Description | Commit | Files |
|------|-------------|--------|-------|
| 1 | Server allowlist poll loop + LLM process classifier | 22f084a | kiosk.rs, main.rs |
| 2 | E2E test script for kiosk allowlist API CRUD | 8ca743f | kiosk-allowlist.sh |

## What Was Built

### Task 1: Server Allowlist + LLM Classifier (kiosk.rs + main.rs)

**kiosk.rs additions:**
- `pub const ALLOWLIST_REFRESH_SECS: u64 = 300` — 5-minute poll interval (exported to main.rs)
- `server_allowlist()` OnceLock following the exact `learned_allowlist()` pattern, starts empty
- `pub fn set_server_allowlist(names: Vec<String>)` — atomically replaces the in-memory server set
- `allowed_set_snapshot()` extended: server allowlist merged as 4th layer (after learned, before temp)
- `PendingClassification { process_name, exe_path }` struct added to `EnforceResult`
- `ProcessVerdict { Allow, Block, Ask }` enum for LLM responses
- `pub async fn classify_process(ollama_url, ollama_model, process_name, exe_path) -> ProcessVerdict` — queries Ollama with 10s timeout, parses BLOCK/ALLOW/ASK from response, defaults to Ask on failure

**main.rs additions:**
- `async fn fetch_server_allowlist(client, base_url) -> anyhow::Result<Vec<String>>` — GET /api/v1/config/kiosk-allowlist, extracts process_name array
- `async fn allowlist_poll_loop(core_http_url, client)` — polls every 5 min with MissedTickBehavior::Skip
- HTTP URL derived from WebSocket URL: `ws://host:port/ws/agent` → `http://host:port`
- Poll loop spawned after billing_guard, before remote_ops await (concurrent with startup)
- kiosk_interval handler updated: spawn_blocking now returns pending_classifications; outer async fires tokio::spawn LLM classification for each new unknown process
- ALLOW verdict: auto-approves + sends ProcessApprovalRequest to server
- BLOCK verdict: calls reject_process()
- ASK verdict: no-op (existing temp-allow + approval flow handles it)

### Task 2: E2E Test Script (tests/e2e/api/kiosk-allowlist.sh)

8-gate CRUD cycle following exact billing.sh pattern:
- Gate 1: API health check (fail-fast gate)
- Gate 2: GET returns allowlist array with hardcoded_count > 0
- Gate 3: POST add test_phase48_dummy.exe returns 201 with id and process_name
- Gate 4: GET verify entry appears in list after POST
- Gate 5: POST duplicate returns 200 or 201 (INSERT OR IGNORE behavior)
- Gate 6: POST svchost.exe returns already_in_baseline response
- Gate 7: DELETE returns 204
- Gate 8: GET verify entry gone after DELETE
- Cleanup trap: always deletes test entry on EXIT to avoid DB pollution

## Verification Results

- `cargo build -p rc-agent-crate`: 0 errors, 27 pre-existing warnings (unchanged)
- `cargo test -p rc-agent-crate`: 250 passed, 0 failed
- `bash -n tests/e2e/api/kiosk-allowlist.sh`: Syntax OK

## Deviations from Plan

None — plan executed exactly as written.

The only structural note: `enforce_process_whitelist_blocking` is called via `spawn_blocking` which returns a JoinHandle. The plan specified firing LLM classification from the kiosk tick handler "after the spawn_blocking call". This was implemented by awaiting the JoinHandle in the outer async select branch (getting `result.pending_classifications`), then firing `tokio::spawn` for each classification. This correctly keeps classification async and non-blocking relative to the enforce loop.

## Self-Check: PASSED

- kiosk.rs: FOUND
- main.rs: FOUND
- kiosk-allowlist.sh: FOUND
- 48-02-SUMMARY.md: FOUND
- Commit 22f084a: FOUND
- Commit 8ca743f: FOUND
