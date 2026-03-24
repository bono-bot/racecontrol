---
phase: 178-agent-sentry-consumer
plan: "03"
subsystem: shared-types
tags: [typescript, contract-tests, ws-messages, v22.0, SYNC-03]
dependency_graph:
  requires: []
  provides: [WS message TypeScript interfaces, WS message contract tests]
  affects: [admin dashboard, web dashboard, kiosk, racingpoint-admin]
tech_stack:
  added: []
  patterns: [contract-test fixture pattern, TS/Rust field agreement verification]
key_files:
  created:
    - packages/shared-types/src/ws-messages.ts
    - packages/contract-tests/src/ws-messages.contract.test.ts
    - packages/contract-tests/src/fixtures/ws-messages.json
  modified:
    - packages/shared-types/src/index.ts
key_decisions:
  - "Used WsConfigPushPayload instead of ConfigPushPayload to avoid collision with existing ConfigPush queue entry type in config.ts"
  - "Fixtures use exact Rust serde snake_case field names to ensure contract tests catch any naming drift"
metrics:
  duration_minutes: 2
  tasks_completed: 2
  tasks_total: 2
  files_created: 3
  files_modified: 1
  completed_date: "2026-03-24"
requirements: [SYNC-03]
---

# Phase 178 Plan 03: WS Message TypeScript Interfaces and Contract Tests Summary

TypeScript interfaces for all 7 v22.0 WebSocket message payloads added to shared-types, with contract tests verifying field agreement against rc-common Rust structs via fixtures.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Create ws-messages.ts TypeScript interfaces and export from index | 2b46b8c4 | packages/shared-types/src/ws-messages.ts, packages/shared-types/src/index.ts |
| 2 | Create contract test with fixtures verifying TS-Rust field agreement | d592ca2d | packages/contract-tests/src/ws-messages.contract.test.ts, packages/contract-tests/src/fixtures/ws-messages.json |

## What Was Built

### packages/shared-types/src/ws-messages.ts
7 TypeScript interfaces mirroring rc-common Rust payload structs exactly:
- `FlagSyncPayload` — server->agent flag sync (flags: Record<string, boolean>, version: number)
- `WsConfigPushPayload` — server->agent config push (fields, schema_version, sequence)
- `OtaDownloadPayload` — server->agent OTA download command (manifest_url, binary_sha256, version)
- `KillSwitchPayload` — server->agent kill switch (flag_name, active, reason?)
- `ConfigAckPayload` — agent->server config acknowledgement (pod_id, sequence, accepted)
- `OtaAckPayload` — agent->server OTA acknowledgement (pod_id, version, success, error?)
- `FlagCacheSyncPayload` — agent->server cache sync request (pod_id, cached_version)

### packages/contract-tests/src/ws-messages.contract.test.ts
10 contract tests under the `WebSocket Message Payloads - TS/Rust contract (SYNC-03)` describe block:
- 7 type assertion tests (one per payload type)
- 3 field-name presence tests (FlagSync, ConfigPush, KillSwitch)

### packages/contract-tests/src/fixtures/ws-messages.json
Sample payloads with exact Rust serde field names (snake_case). OtaAck fixture omits optional `error` field to verify optional field handling.

## Verification

- `tsc --noEmit` passes in shared-types (0 errors)
- All 31 contract tests pass: 24 existing + 10 new ws-messages tests
- All 7 payload type interfaces present in ws-messages.ts
- All 7 exported from index.ts

## Deviations from Plan

None — plan executed exactly as written.

## Self-Check: PASSED

- FOUND: packages/shared-types/src/ws-messages.ts
- FOUND: packages/contract-tests/src/ws-messages.contract.test.ts
- FOUND: packages/contract-tests/src/fixtures/ws-messages.json
- FOUND: commit 2b46b8c4 (Task 1)
- FOUND: commit d592ca2d (Task 2)
