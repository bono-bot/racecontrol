# Roadmap: v12.1 E2E Process Guard

## Overview

Five phases converting the deny-by-default whitelist concept into a running enforcement daemon across all 11 Racing Point machines. Build order is dictated by compile-time dependencies: rc-common protocol types must exist before either racecontrol or rc-agent can reference them. The whitelist schema is designed before any enforcement code is written. The server endpoint goes live before pods attempt to fetch the whitelist. Pods deploy in report-only mode for whitelist tuning before kills are enabled. Port audit, scheduled task audit, and the James standalone binary ship last as secondary enforcement vectors.

## Phases

**Phase Numbering:** Starting at 101 (continuation of unified phase numbering)

- [ ] **Phase 101: Protocol Foundation** - New rc-common types and AgentMessage variants that rc-agent and racecontrol depend on at compile time
- [ ] **Phase 102: Whitelist Schema + Config + Fetch Endpoint** - Central whitelist in racecontrol.toml with per-machine overrides and the HTTP endpoint pods use to fetch their merged whitelist
- [x] **Phase 103: Pod Guard Module** - rc-agent process_guard.rs covering process scan, auto-kill, Run key and Startup folder audit, audit log, and fleet reporting
- [ ] **Phase 104: Server Guard Module + Alerts** - racecontrol process_guard.rs receiving violations, kiosk notification badge, email escalation, and fleet health integration
- [ ] **Phase 105: Port Audit + Scheduled Tasks + James Binary** - Listening port enforcement, scheduled task audit, and standalone rc-process-guard binary for James workstation

## Phase Details

### Phase 101: Protocol Foundation
**Goal**: rc-common compiles with all types and message variants that downstream crates need for process guard integration
**Depends on**: Nothing (first phase)
**Requirements**: GUARD-04, GUARD-05
**Success Criteria** (what must be TRUE):
  1. `cargo test -p rc-common` passes with zero warnings after adding new types
  2. `MachineWhitelist`, `ViolationType`, and `ProcessViolation` structs are importable from rc-common in a test
  3. `AgentMessage::ProcessViolation`, `AgentMessage::ProcessGuardStatus`, and `CoreToAgentMessage::UpdateProcessWhitelist` variants exist in rc-common/src/protocol.rs and serialize/deserialize correctly via serde
  4. Neither racecontrol nor rc-agent require changes to compile after rc-common is updated (no breaking changes to existing variants)
**Plans**: 1 plan
Plans:
- [x] 101-01-PLAN.md — Add MachineWhitelist/ViolationType/ProcessViolation types to rc-common/src/types.rs and three protocol variants to rc-common/src/protocol.rs; TDD with 9 tests; confirm downstream compile

### Phase 102: Whitelist Schema + Config + Fetch Endpoint
**Goal**: Staff can open racecontrol.toml and see a populated deny-by-default process whitelist with per-machine sections, and any pod can curl the fetch endpoint to receive its merged whitelist
**Depends on**: Phase 101
**Requirements**: GUARD-01, GUARD-02, GUARD-03, GUARD-06
**Success Criteria** (what must be TRUE):
  1. `racecontrol.toml` contains a `[process_guard]` section with a global whitelist that includes every known-legitimate process on Racing Point machines
  2. Per-machine override sections exist (`[process_guard.overrides.james]`, `[process_guard.overrides.pod]`, `[process_guard.overrides.server]`) with correct entries (James gets Ollama, pods do not get Steam)
  3. Whitelist entries carry category tags (system, racecontrol, game, peripheral, ollama) and wildcard/prefix patterns compile without panics
  4. `curl http://192.168.31.23:8080/api/v1/guard/whitelist/pod-8` returns a valid JSON MachineWhitelist with global entries merged with pod overrides
  5. `violation_action` defaults to `"report_only"` in the TOML so no kills happen on first deploy
**Plans**: 2 plans
Plans:
- [x] 102-01-PLAN.md — ProcessGuardConfig structs in racecontrol/src/config.rs (AllowedProcess, ProcessGuardOverride, ProcessGuardConfig) + racecontrol.toml populated with 185 global entries and 3 per-machine override sections
- [x] 102-02-PLAN.md — racecontrol/src/process_guard.rs with merge_for_machine() logic and GET /api/v1/guard/whitelist/{machine_id} endpoint; wired into lib.rs and api/routes.rs

### Phase 103: Pod Guard Module
**Goal**: All 8 pods run a background process guard that scans every 60 seconds, kills confirmed violations after two consecutive scan cycles, removes non-whitelisted Run keys and Startup shortcuts, and streams every violation to the server via WebSocket
**Depends on**: Phase 102
**Requirements**: PROC-01, PROC-02, PROC-03, PROC-04, PROC-05, AUTO-01, AUTO-02, AUTO-04, ALERT-01, ALERT-04, DEPLOY-01
**Success Criteria** (what must be TRUE):
  1. A non-whitelisted process started on Pod 8 during report-only mode appears in `C:\RacingPoint\process-guard.log` within 70 seconds (two scan cycles) with correct machine ID, PID, process name, and timestamp
  2. When `violation_action` is set to `"kill_and_report"`, the same non-whitelisted process is terminated and a `ProcessViolation` WebSocket message reaches the server — rc-agent.exe and guard itself are never in the kill list
  3. A non-whitelisted HKCU Run key added to a pod is removed within 5 minutes and logged; a backup file is written before removal
  4. Pod binary guard detects `racecontrol.exe` running on a pod and emits a CRITICAL severity violation with zero grace period
  5. Process audit log rotates at 512KB without data loss and without crashing rc-agent
**Plans**: 3 plans
Plans:
- [x] 103-01-PLAN.md — ProcessGuardConfig in config.rs + walkdir dep + guard_whitelist/guard_violation_tx/rx fields in AppState and main.rs
- [x] 103-02-PLAN.md — core process_guard.rs module: scan loop, grace period, taskkill, PID identity, CRITICAL zero-grace, log rotation, AgentMessage::ProcessViolation dispatch
- [x] 103-03-PLAN.md — autostart audit (Run keys + Startup folder) added to process_guard.rs + whitelist fetch on WS connect in main.rs + guard_violation_rx drain in event_loop.rs + UpdateProcessWhitelist handler in ws_handler.rs

### Phase 104: Server Guard Module + Alerts
**Goal**: The racecontrol server receives all pod violations, displays an active-violation badge on the staff kiosk, escalates repeat offenders to email, and surfaces violation counts in the fleet health endpoint
**Depends on**: Phase 103
**Requirements**: ALERT-02, ALERT-03, ALERT-05, DEPLOY-02
**Success Criteria** (what must be TRUE):
  1. Staff kiosk shows a notification badge when any pod has an active unacknowledged violation — badge clears when violations are resolved
  2. `GET /api/v1/fleet/health` response includes `violation_count_24h` and `last_violation_at` fields for each pod
  3. A process killed three or more times within a 5-minute window on any pod triggers an email to Uday with the machine ID, process name, and kill count
  4. racecontrol's own process guard module runs on server .23, logs to `C:\RacingPoint\process-guard.log`, and reports CRITICAL if rc-agent.exe is detected running on the server
**Plans**: 3 plans
Plans:
- [x] 104-01-PLAN.md — ViolationStore in fleet_health.rs + pod_violations field on AppState + ProcessViolation WS handler + fleet/health violation fields + email escalation on repeat offenders
- [x] 104-02-PLAN.md — spawn_server_guard() in process_guard.rs + sysinfo scan loop on server + rc-agent.exe CRITICAL detection + wired into main.rs
- [ ] 104-03-PLAN.md — PodFleetStatus TypeScript type updated + violation badge on kiosk fleet grid (Racing Red #E10600)

### Phase 105: Port Audit + Scheduled Tasks + James Binary
**Goal**: Listening ports are audited against the approved port list, non-whitelisted scheduled tasks are flagged, and James workstation runs a standalone rc-process-guard binary that reports via HTTP instead of WebSocket (standing rule: never run pod binaries on James)
**Depends on**: Phase 104
**Requirements**: PORT-01, PORT-02, AUTO-03, DEPLOY-03
**Success Criteria** (what must be TRUE):
  1. A process binding a non-whitelisted listening port on any pod is killed within one scan cycle and the violation is logged with port number and PID
  2. A non-whitelisted scheduled task on a pod is flagged in the audit log with task name, task path, and action taken (LOG or REMOVE per enforcement stage)
  3. `rc-process-guard.exe` runs on James .27, scans on the same interval as rc-agent guard, and POSTs `ProcessViolation` payloads to `http://192.168.31.23:8080/api/v1/guard/report` via Tailscale — it never connects via WebSocket
  4. The James whitelist covers Ollama, node, python, VS Code, comms-link, cargo, and deploy tooling without false positives on the first run
**Plans**: TBD

## Progress

**Execution Order:** 101 → 102 → 103 → 104 → 105

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 101. Protocol Foundation | 1/1 | Complete | 2026-03-21 |
| 102. Whitelist Schema + Config + Fetch Endpoint | 2/2 | Complete | 2026-03-21 |
| 103. Pod Guard Module | 3/3 | Complete | 2026-03-21 |
| 104. Server Guard Module + Alerts | 0/3 | Not started | - |
| 105. Port Audit + Scheduled Tasks + James Binary | 0/TBD | Not started | - |

---
*Roadmap created: 2026-03-21 IST*
*Milestone: v12.1 E2E Process Guard*
