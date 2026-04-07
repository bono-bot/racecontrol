# Roadmap: RaceControl Ops

## Milestones

- ✅ **v1.0** — Phases 01-36 (shipped)
- ✅ **v10.0** — Phases 41-50 (shipped)
- ✅ **v11.0** — Phases 51-60 (shipped)
- ✅ **v16.1** — Camera Dashboard Pro (shipped)
- ✅ **v17.1** — Phases 66-80 (shipped)
- ✅ **v21.0** — Cross-Project Sync (shipped)
- ✅ **v25.0** — Phases 81-96 (shipped)
- ✅ **v32.0 Autonomous Meshed Intelligence** — Phases 273-279 (shipped 2026-04-01)
- ✅ **v35.0 Structured Retraining & Model Lifecycle** — Phases 290-294 (shipped 2026-04-01)
- ✅ **v38.0 Security Hardening & Operational Maturity** — Phases 305-309 (shipped 2026-04-02)
- ✅ **v39.0 Session Trace ID & Metrics** — Phase 310 (shipped 2026-04-02)
- ✅ **v40.0 Game Launch Reliability** — Phases 311-314 (shipped 2026-04-03)
- ✅ **v41.0 Game Intelligence System** — Phases 315-320 (shipped 2026-04-03)
- ✅ **v43.0 Self-Audit & Visual Regression System** — Phases 325-328 (shipped 2026-04-06)
- ✅ **v42.0 Meshed Intelligence Migration** — Phases 321-324 (shipped 2026-04-07)
- [x] **v44.0 VMS Architecture Integration** — Phases 329-336 (8 phases, shipped 2026-04-07)
- [ ] **v45.0 Credits/Rupees Wallet Separation** — Phases 337-342 (6 phases, financial model redesign)
See `.planning/milestones/` for archived roadmaps and requirements per milestone.

---

## v40.0 Game Launch Reliability

**Goal:** Fix 4 critical architectural issues in the game launch workflow — WS ACK protocol, GameState loss prevention, billing lock race, billing-during-launch guard.

**Phases:** 4  |  **Coverage:** 12/12 requirements mapped

**Dependency graph:**
```
311 (Launch-Billing Guard) ──> 312 (WS ACK Protocol) ──> 313 (GameState Resilience)
                                                                       │
                                                          314 (Billing Atomicity)
```

### Phases

- [x] **Phase 311: Launch-Billing Coordination Guard** — LBILL-01, LBILL-02, LBILL-03
- [x] **Phase 312: WS ACK Protocol** — WSCMD-01, WSCMD-02, WSCMD-03, WSCMD-04
- [x] **Phase 313: Game State Resilience** — GSTATE-01, GSTATE-02, GSTATE-03
- [x] **Phase 314: Billing Atomicity** — BATOM-01, BATOM-02

---

### Phase 311: Launch-Billing Coordination Guard

**Goal:** Prevent the 5-min stale cancel from killing sessions where the game is actively loading. Customer shouldn't play for free if billing is cancelled but game keeps running.

**Requirements:** LBILL-01, LBILL-02, LBILL-03

**Success criteria:**
1. When game process is alive on pod but AcStatus::Live not yet received, stale cancel is deferred (not executed)
2. If game is alive >10 min without Live signal, cancel proceeds with refund (graceful timeout)
3. If game is dead and session is waiting_for_game >5 min, cancel with full wallet refund
4. Log every stale cancel decision with reason (game_alive/game_dead/extended)

**Plans:** 1 plan

Plans:
- [x] 311-01-PLAN.md -- Game-aware stale cancel: check GameTracker before cancelling waiting_for_game sessions (4488f48a)

**Key files:**
- `crates/racecontrol/src/billing.rs` — tick_all_timers stale cancel logic (line ~1442)
- `crates/racecontrol/src/game_launcher.rs` — GameTracker state query
- `crates/rc-common/src/protocol.rs` — may need IsGameAlive query message

---

### Phase 312: WS ACK Protocol

**Goal:** Server commands to agents are confirmed-delivery, not fire-and-forget. Launch and stop return success only after agent acknowledges receipt.

**Requirements:** WSCMD-01, WSCMD-02, WSCMD-03, WSCMD-04

**Success criteria:**
1. `/games/launch` returns `{"ok":true}` only after agent ACKs (or `{"ok":false,"error":"timeout"}` after 5s)
2. `/games/stop` returns `{"ok":true}` only after agent ACKs
3. Old agents (pre-v40) that don't send ACK hit the 5s timeout — server returns error, no crash
4. ACK messages are a new `AgentMessage::CommandAck { command_id }` variant

**Key files:**
- `crates/racecontrol/src/api/routes.rs` — launch_game, stop_game endpoints
- `crates/racecontrol/src/game_launcher.rs` — handle_dashboard_command
- `crates/rc-common/src/protocol.rs` — AgentMessage, CoreToAgentMessage
- `crates/rc-agent/src/ws_handler.rs` — command handlers (send ACK after processing)
- `crates/racecontrol/src/ws/mod.rs` — receive ACK and resolve waiting future

**Plans:** 1 plan

Plans:
- [x] 312-01-PLAN.md -- CommandAck protocol: agent ACKs for launch/stop, server 5s timeout wait (b7359a02)

---

### Phase 313: Game State Resilience

**Goal:** GameTracker never gets permanently stuck. WS reconnects don't create phantom state.

**Requirements:** GSTATE-01, GSTATE-02, GSTATE-03

**Success criteria:**
1. GameTracker in `Launching` for >3 min auto-transitions to `Error` (background timeout task)
2. On WS reconnect, server queries agent for current game state and updates tracker accordingly
3. After successful `/games/stop`, GameTracker entry is removed (not left in `Stopping`)
4. No pod can be permanently blocked from launching games due to stale tracker state

**Key files:**
- `crates/racecontrol/src/game_launcher.rs` — GameTracker, timeout logic, stop cleanup
- `crates/racecontrol/src/ws/mod.rs` — reconnect reconciliation

**Plans:** 1 plan

Plans:
- [x] 313-01-PLAN.md -- GSTATE-01/02/03: Launching hard-cap timeout, smart WS reconciliation, stop ACK cleanup (c0219f30, eb0db70b)

---

### Phase 314: Billing Atomicity

**Goal:** No concurrent request window can create duplicate billing sessions for the same pod.

**Requirements:** BATOM-01, BATOM-02

**Success criteria:**
1. Two simultaneous `start_billing` requests for the same pod: exactly one succeeds, one gets clear error
2. Pre-validation check and session INSERT are atomic (no TOCTOU window)
3. Existing `active_timers` pre-check AND DB UNIQUE constraint both prevent duplicates (defense in depth)

**Plans:** 1 plan

Plans:
- [x] 314-01-PLAN.md -- Per-pod billing start lock + dual pre-validation (active_timers + waiting_for_game)

**Key files:**
- `crates/racecontrol/src/api/routes.rs` — start_billing function
- `crates/racecontrol/src/billing.rs` — active_timers map

---

## v41.0 Game Intelligence System

**Goal:** Proactive game availability management and launch failure observability — stop showing customers games they can't play, flag broken AC combos before launch, and surface failures instantly through Meshed Intelligence.

**Phases:** 6  |  **Coverage:** 17/17 requirements mapped

**Dependency graph:**
```
315 (Shared Types Foundation)
  └──> 316 (Agent Content Scanner & Boot Validation)
         └──> 317 (Server Inventory & Fleet Intelligence)
                └──> 318 (Launch Intelligence)
                └──> 319 (Reliability Dashboard)
                └──> 320 (Kiosk Game Filtering)
```

### Phases

- [x] **Phase 315: Shared Types Foundation** — LAUNCH-02
- [x] **Phase 316: Agent Content Scanner & Boot Validation** — INV-01, INV-04, COMBO-01, COMBO-02
 (completed 2026-04-03)
- [x] **Phase 317: Server Inventory & Fleet Intelligence** — INV-02, COMBO-03, COMBO-04, LAUNCH-03, LAUNCH-04 (completed 2026-04-03)
- [x] **Phase 318: Launch Intelligence** — LAUNCH-01, LAUNCH-05 (completed 2026-04-03)
- [x] **Phase 319: Reliability Dashboard** — DASH-01, DASH-02, DASH-03 (completed 2026-04-03)
- [x] **Phase 320: Kiosk Game Filtering** — INV-03, COMBO-05 (completed 2026-04-03)

---

### Phase 315: Shared Types Foundation

**Goal:** Add rc-common types for all v41.0 data contracts — game inventory, combo validation, launch timelines, combo health, crash loop detection.

**Plans:** 1/1 plans complete

Plans:
- [x] 315-01-PLAN.md -- v41.0 game intelligence shared types: InstalledGame, GameInventory, ComboValidationResult, LaunchTimeline, ComboHealthSummary, CrashLoopReport (4e6a2717)

---

### Phase 316: Agent Content Scanner & Boot Validation
**Goal**: rc-agent auto-detects all installed games (Steam + non-Steam) at boot and proactively validates AC combos against the filesystem before any customer session starts
**Depends on**: Phase 315
**Requirements**: INV-01, INV-04, COMBO-01, COMBO-02
**Success Criteria** (what must be TRUE):
  1. After pod boot, the server receives a `GameInventoryUpdate` WS message listing all installed SimTypes — including Steam games detected via `libraryfolders.vdf` parsing (not hardcoded paths only)
  2. Every 5 minutes, rc-agent rescans and sends a fresh `GameInventoryUpdate` — the server's pod inventory reflects changes within one scan cycle without pod restart
  3. After receiving the first preset push from the server, rc-agent sends `ComboValidationResult` messages for each AC preset — each result includes whether car folder, track folder, and AI lines exist on that pod
  4. Combo validation log shows "Presets received" before "Combo validation complete" — validation does not run against an empty preset list if the server is slow at boot
  5. A game installed to a non-default Steam library path (D:\ or E:\) appears in the inventory scan result
**Plans**: 2 plans

Plans:
- [x] 316-01-PLAN.md -- Steam VDF library scanning + non-Steam game exe probing + GameInventoryUpdate WS send + 5-min periodic rescan loop
- [x] 316-02-PLAN.md -- validate_ac_combo/validate_ac_combos (car/track/ai checks) + PresetPush handler gate + ComboValidationReport WS send

### Phase 317: Server Inventory & Fleet Intelligence
**Goal**: The server persists per-pod game inventory and combo validation results, aggregates fleet availability, auto-disables universally broken combos, and alerts staff on crash loops and chain launch failures
**Depends on**: Phase 316
**Requirements**: INV-02, COMBO-03, COMBO-04, LAUNCH-03, LAUNCH-04
**Success Criteria** (what must be TRUE):
  1. After a pod sends `GameInventoryUpdate`, rows exist in `pod_game_inventory` for that pod — data survives server restart and shows the last scan result for any pod that has connected
  2. Fleet combo aggregation categorizes each AC preset as: valid (installed on all pods), partial (some pods), or invalid (no pods) — visible via `GET /api/v1/presets` which includes a `fleet_validity` field
  3. An AC preset that is invalid on ALL pods has `enabled = false` set in `game_presets` and a WhatsApp alert fires to staff naming the preset and the missing filesystem component
  4. A pod sending more than 3 `StartupReport` messages in 5 minutes with `uptime_secs < 30` produces `crash_loop: true` in `/api/v1/fleet/health`, an ERROR-level server log, and a WhatsApp alert naming the pod and restart count
  5. Three consecutive game launch failures for the same pod and SimType within 10 minutes trigger an `EscalationRequest` WS message routed to WhatsApp — Uday receives a message naming the pod and game
**Plans**: 2 plans

Plans:
- [x] 317-01-PLAN.md -- game_inventory.rs (pod_game_inventory + combo_validation_flags tables, upsert fns, fleet_validity, auto-disable), WS handlers for GameInventoryUpdate + ComboValidationReport, fleet_validity in GET /api/v1/presets
- [x] 317-02-PLAN.md -- crash loop WhatsApp fix (EscalationRequest path), ChainFailureState in AppState, chain failure detection in GameStateUpdate handler

### Phase 318: Launch Intelligence
**Goal**: Every game launch has a timeout watchdog that prevents permanent pod lockout and records step-level timeline spans so launch failures can be debugged at the exact checkpoint where they stalled
**Depends on**: Phase 317, Phase 312 (v40.0 WS ACK — confirmed deployed b7359a02)
**Requirements**: LAUNCH-01, LAUNCH-05
**Success Criteria** (what must be TRUE):
  1. If a game process does not reach playable state within 90 seconds (default) after launch, `GameTracker` auto-transitions to `Error` state and `DiagnosticTrigger::GameLaunchTimeout` is emitted to the tier engine channel
  2. After any launch (success or failure), rows exist in `launch_timeline_spans` for at least `ws_sent`, `agent_received`, `process_spawned`, and `playable_signal` checkpoints — each with millisecond-resolution elapsed time
  3. A combo with historical p95 launch time under 45 seconds receives a shorter timeout than the 90-second default — configurable via AgentConfig push from server
  4. Timeline span data is returned by `GET /api/v1/launch-timeline/{launch_id}` within one second of launch completion
**Plans**: 2 plans

Plans:
- [x] 318-01-PLAN.md -- LaunchTimedOut WS message server→agent, GameLaunchTimeout DiagnosticTrigger variant, launch_timeout_config in AgentConfig, emit from check_game_health
- [x] 318-02-PLAN.md -- launch_timeline_spans table migration, GameTracker launch_id, agent LaunchTimelineReport send, server WS handler + GET /api/v1/launch-timeline/{launch_id}

### Phase 319: Reliability Dashboard
**Goal**: Staff can see at a glance which pods have which games installed, which AC combos are flagged unreliable, and drill into any specific launch incident to find the checkpoint where it stalled
**Depends on**: Phase 317, Phase 318
**Requirements**: DASH-01, DASH-02, DASH-03
**Success Criteria** (what must be TRUE):
  1. Opening `/reliability` in the admin dashboard shows an 8-pod x 8-game matrix with install status badges (installed / not installed) sourced live from `pod_game_inventory`
  2. The reliability page shows per-combo success rates sortable by rate, with combos below a configurable threshold highlighted in red — data refreshes within 30 seconds of a new launch event
  3. Clicking any combo row expands it to show the most recent launch timeline — checkpoint timestamps visible for ws_sent, agent_received, process_spawned, and playable_signal
  4. The dashboard loads in under 3 seconds when opened from James's machine (not from the server itself) — static files serve correctly from a remote browser
**Plans**: 2 plans

Plans:
- [x] 319-01-PLAN.md -- Fleet game matrix (GET /api/v1/fleet/game-matrix from pod_game_inventory) + combo reliability table (GET /api/v1/admin/combo-list from combo_reliability, sortable, red highlight < 70%) added to /games/reliability page
- [x] 319-02-PLAN.md -- Launch timeline viewer at /games/timeline: GET /api/v1/launch-timeline/recent endpoint + expandable per-launch detail with checkpoint timestamps

### Phase 320: Kiosk Game Filtering
**Goal**: Customers on each pod only see games and AC combos that are actually available on that specific pod — no silent launch failures from showing unavailable content
**Depends on**: Phase 317
**Requirements**: INV-03, COMBO-05
**Success Criteria** (what must be TRUE):
  1. On the kiosk at Pod 3, a SimType absent from Pod 3's `pod_game_inventory` does not appear in the game selection screen — verified by opening the kiosk in a browser from James's machine pointed at a Pod 3 session
  2. AC presets with `combo_valid: false` for the current pod are either hidden or shown with an "Unavailable" badge — the customer cannot launch an unlaunchable combo
  3. The kiosk game list reflects inventory changes within 30 seconds of a new `GameInventoryUpdate` being processed by the server
  4. The kiosk does not flicker or re-render mid-browse when inventory updates arrive — changes apply only between sessions or after a debounce interval
**Plans**: 1 plan

Plans:
- [x] 320-01-PLAN.md -- GET /api/v1/fleet/pod-inventory/{pod_id} (server) + kiosk game picker from server inventory + Unavailable badge on invalid AC combos (INV-03, COMBO-05)

---

### Previous Milestone Phases (archived)

- [x] **Phase 305: TLS for Internal HTTP** — Self-signed venue CA, mTLS on :8080/:8090, Tailscale bypass ✅ (2026-04-01)
- [x] **Phase 306: WS Auth Hardening** — Per-pod JWT (24h), auto-rotation, invalid = disconnect + alert ✅ (b33e388e)
- [x] **Phase 307: Audit Log Integrity** — SHA-256 hash chain, tamper detection, verify endpoint (d5f9b387)
- [x] **Phase 308: RBAC for Admin** — cashier/manager/superadmin roles, JWT claims, endpoint enforcement ✅ (pre-built)
- [x] **Phase 309: Security Audit Script** — Automated scan, JSON scorecard, gate-check integration ✅ (2026-04-02)

### Phase 301: Cloud Data Sync v2
**Goal**: Key intelligence tables are synced to Bono VPS and the system is ready for cross-venue data flows
**Depends on**: Phase 300
**Requirements**: SYNC-01, SYNC-02, SYNC-03, SYNC-04, SYNC-05, SYNC-06
**Success Criteria** (what must be TRUE):
  1. fleet_solutions, model_evaluations, and metrics_rollups rows written at the venue appear in the Bono VPS database within the next sync cycle (server-authoritative direction)
  2. A row written with a future venue_id on Bono VPS flows back to the venue database on the next sync (cloud-authoritative direction established)
  3. When two writes target the same row, the row with the later updated_at timestamp wins; if timestamps are equal, the row with the lexicographically smaller venue_id wins
  4. Admin dashboard sync panel shows last sync timestamp, number of tables synced, and running conflict count
**Plans:** 1/1 plans complete

Plans:
- [x] 301-01-PLAN.md -- DB migrations + cloud_sync.rs push/receive/pull for fleet_solutions, model_evaluations, metrics_rollups with LWW conflict resolution
- [x] 301-02-PLAN.md -- Admin settings Sync Status panel (syncHealth API client + SyncStatusPanel component)

### Phase 302: Structured Event Archive
**Goal**: Every significant system event is captured, queryable, and permanently archived off-server
**Depends on**: Phase 300
**Requirements**: EVENT-01, EVENT-02, EVENT-03, EVENT-04, EVENT-05
**Success Criteria** (what must be TRUE):
  1. After any significant system action (session start/end, deploy, alert fire, pod recovery), a row appears in the events table with type, source, pod, timestamp, and JSON payload populated
  2. A JSONL file for the previous day's events exists in the archive directory by 01:00 IST each morning
  3. Events in SQLite older than 90 days are purged by the daily maintenance task; the corresponding JSONL files remain untouched
  4. The nightly JSONL file for the previous day appears on Bono VPS after the archive task runs
  5. GET /api/v1/events returns a filtered list of events when given type, pod, or date range query parameters
**Plans:** 2/2 plans complete

Plans:
- [x] 302-01-PLAN.md -- EventArchiveConfig, system_events table, event_archive.rs (append_event, spawn, export, purge, SCP), wired into main.rs
- [x] 302-02-PLAN.md -- GET /api/v1/events REST handler with filters, instrument 6 high-signal event sources with append_event calls

### Phase 303: Multi-Venue Schema Prep
**Goal**: The database schema supports a second venue without data model changes -- only a config value changes
**Depends on**: Phase 301, Phase 302
**Requirements**: VENUE-01, VENUE-02, VENUE-03, VENUE-04
**Success Criteria** (what must be TRUE):
  1. Every major table has a venue_id column; existing rows all have venue_id = 'racingpoint-hyd-001' and the application behaves identically to before the migration
  2. The migration runs on an existing production database without data loss -- no manual intervention required, no functional behavior change for current single-venue operation
  3. All INSERT and UPDATE queries in racecontrol pass venue_id explicitly -- no row is written without a venue_id value
  4. MULTI-VENUE-ARCHITECTURE.md exists and documents the trigger conditions, schema strategy, sync model, and breaking points for a second venue
**Plans**: 2 plans

Plans:
- [x] 303-01-PLAN.md -- VenueConfig venue_id field, ALTER migrations for 44 tables, MULTI-VENUE-ARCHITECTURE.md design doc
- [x] 303-02-PLAN.md -- Add venue_id to ~122 INSERT statements across 22 source files

### Phase 304: Fleet Deploy Automation
**Goal**: Staff can deploy a new binary to the entire fleet in one API call with automatic safety gates
**Depends on**: Phase 303
**Requirements**: DEPLOY-01, DEPLOY-02, DEPLOY-03, DEPLOY-04, DEPLOY-05, DEPLOY-06
**Success Criteria** (what must be TRUE):
  1. POST /api/v1/fleet/deploy with a binary hash and scope (all/canary/specific pods) initiates a deployment and returns a deploy_id immediately
  2. The deploy goes to Pod 8 first; the next wave does not start until Pod 8 passes its health check
  3. After canary passes, remaining pods receive the binary in waves with a configurable inter-wave delay; the full fleet is updated without additional manual action
  4. If Pod 8 or any subsequent wave pod fails its post-deploy health check, all affected pods are automatically reverted to the previous binary
  5. GET /api/v1/fleet/deploy/status shows current wave, each pod's status (pending/deploying/healthy/rolled-back), and a log of rollback events
  6. No pod swaps its binary while it has an active billing session; the swap is deferred until the session ends naturally
**Plans**: 2 plans

Plans:
- [x] 304-01-PLAN.md -- FleetDeploySession types, run_fleet_deploy orchestration, wave/rollback/billing logic, unit tests
- [x] 304-02-PLAN.md -- AppState field, route handlers (POST /fleet/deploy + GET /fleet/deploy/status), superadmin route registration

## Progress

**Execution Order:**
295 -> 296 -> 297
296 -> 298
296 -> 299
300 -> 301
300 -> 302
301 + 302 -> 303
303 -> 304

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 285. Metrics Ring Buffer | 2/2 | Complete | 2026-04-01 |
| 286. Metrics Query API | 1/1 | Complete | 2026-04-01 |
| 287. Metrics Dashboard | 1/1 | Complete | 2026-04-01 |
| 288. Prometheus Export | 1/1 | Complete | 2026-04-01 |
| 289. Metric Alert Thresholds | 2/2 | Complete | 2026-04-01 |
| 290. Wire Metric Producers | 1/1 | Complete | 2026-04-01 |
| 291. Dashboard API Wiring | 1/1 | Complete | 2026-04-01 |
| 295. Config Schema & Validation | 1/1 | Complete | 2026-04-01 |
| 296. Server-Pushed Config | 2/2 | Complete | 2026-04-01 |
| 297. Config Editor UI | 2/2 | Complete | 2026-04-01 |
| 298. Game Preset Library | 2/2 | Complete | 2026-04-01 |
| 299. Policy Rules Engine | 0/3 | Complete | 2026-04-01 |
| 300. SQLite Backup Pipeline | 2/2 | Complete | 2026-04-01 |
| 301. Cloud Data Sync v2 | 2/2 | Complete | 2026-04-01 |
| 302. Structured Event Archive | 2/2 | Complete | 2026-04-01 |
| 303. Multi-Venue Schema Prep | 4/1 | Complete | 2026-04-02 |
| 304. Fleet Deploy Automation | 2/2 | Complete | 2026-04-02 |
| 305. TLS for Internal HTTP | 1/1 | Complete | 2026-04-01 |
| 306. WS Auth Hardening | 1/1 | Complete | b33e388e |
| 307. Audit Log Integrity | 1/1 | Complete | d5f9b387 |
| 308. RBAC for Admin | 1/1 | Complete (pre-built) | 2026-04-02 |
| 309. Security Audit Script | 1/1 | Complete | 2026-04-02 |

---

## v38.0 Phase Details

### Phase 305: TLS for Internal HTTP
**Goal**: All internal HTTP traffic between server and agents is encrypted via mutual TLS using a self-signed venue CA
**Depends on**: Nothing (foundation for v38.0)
**Requirements**: TLS-01, TLS-02, TLS-03, TLS-04
**Success Criteria** (what must be TRUE):
  1. `scripts/generate-venue-ca.sh` produces a venue CA cert, server cert, and per-pod client certs in one command
  2. Axum server on :8080 rejects HTTP requests from clients without a valid venue CA cert (returns TLS handshake failure)
  3. rc-agent on :8090 rejects requests from callers without the server's client cert
  4. Connections via Tailscale IP bypass mTLS check (already encrypted end-to-end)
**Plans**: TBD

### Phase 306: WS Auth Hardening
**Goal**: WebSocket connections use short-lived per-pod JWTs instead of static PSK, with automatic rotation and alerts on invalid tokens
**Depends on**: Phase 305 (TLS provides the encrypted channel for JWT exchange)
**Requirements**: WSAUTH-01, WSAUTH-02, WSAUTH-03, WSAUTH-04
**Success Criteria** (what must be TRUE):
  1. Each pod receives a unique JWT with 24-hour expiry after initial PSK-authenticated connection
  2. JWT auto-rotates 1 hour before expiry via a refresh message on the existing WS connection — no reconnection needed
  3. A pod sending an expired or invalid JWT is immediately disconnected and a WhatsApp alert fires to staff
  4. Initial connection still uses PSK (backward compatible) — server issues JWT in the first authenticated response
**Plans**: TBD

### Phase 307: Audit Log Integrity
**Goal**: Every auditable action produces a hash-chained log entry that proves the log hasn't been tampered with
**Depends on**: Phase 305 (TLS secures the API endpoint that verifies the chain)
**Requirements**: AUDIT-01, AUDIT-02, AUDIT-03, AUDIT-04
**Success Criteria** (what must be TRUE):
  1. Each new activity_log entry includes a `previous_hash` field containing the SHA-256 of the immediately preceding entry
  2. If any entry's `previous_hash` doesn't match the computed hash of the previous entry, `GET /api/v1/audit/verify` returns `{valid: false, broken_at: N}`
  3. Config changes, binary deploys, billing start/end, and admin CRUD operations each produce hash-chained audit entries
  4. `GET /api/v1/audit/verify` returns `{valid: true, chain_length: N, last_hash: "..."}` when the chain is intact
**Plans**: TBD

### Phase 308: RBAC for Admin
**Goal**: Staff access is limited by role — a cashier cannot access config or deploy endpoints, a manager cannot modify roles
**Depends on**: Phase 306 (JWT tokens carry the role claim)
**Requirements**: RBAC-01, RBAC-02, RBAC-03, RBAC-04
**Success Criteria** (what must be TRUE):
  1. Three roles exist in the system: cashier, manager, superadmin — stored in a `staff_roles` table
  2. JWT tokens issued to staff include a `role` claim extracted by middleware on every request
  3. A cashier-role JWT calling `POST /api/v1/config/...` or `POST /api/v1/fleet/deploy` receives HTTP 403
  4. Admin dashboard pages for config, deploy, and user management are visible only to manager+ roles (server enforces, UI hides)
**Plans**: TBD

### Phase 309: Security Audit Script
**Goal**: A single command produces a security scorecard covering all v38.0 hardening — integrated into the deploy gate
**Depends on**: Phase 305, Phase 306, Phase 307, Phase 308 (audits everything built in prior phases)
**Requirements**: SECAUDIT-01, SECAUDIT-02, SECAUDIT-03
**Success Criteria** (what must be TRUE):
  1. `bash scripts/security-audit.sh` checks: open ports (only expected ones), TLS config (valid certs, mTLS enforced), JWT validity (not expired, correct claims), default credentials (none found), chain integrity (verify endpoint returns valid)
  2. Output is `security-scorecard.json` with `{checks: [{name, status, details}], score: N/M, overall: pass|fail}`
  3. `gate-check.sh --pre-deploy` includes security-audit.sh — deploy is blocked if overall is `fail`
**Plans**: TBD

---

---

## v39.0 Observability & Session Traceability (Next)

**Goal:** Single-query debugging across the full customer session lifecycle — Launch -> Billing -> Crash -> Refund.

**Phases:** 1 (expandable)

### Phase 310: Session Trace ID Propagation
**Goal**: Every log, metric, and event during a customer session includes `session_id` for end-to-end traceability
**Depends on**: None (additive)
**Requirements**: MI-5 (Mermaid AI finding)
**Success Criteria** (what must be TRUE):
  1. `log_pod_activity()` accepts and persists `session_id` — all callers in billing/launch pass it
  2. `GameTracker` has `billing_session_id` field set when launch is tied to a billing session
  3. `LaunchEvent` metrics include `billing_session_id` for launch/crash correlation
  4. A query on `pod_activity_log WHERE session_id = ?` returns the complete session timeline
**Plans**: 2 plans (core propagation + dashboard events)

Plans:
- [x] 310-01-PLAN.md -- Core: add session_id to activity_log, GameTracker, LaunchEvent (3501828c)
- [ ] 310-02-PLAN.md -- Dashboard events + optional GET /sessions/{id}/trace endpoint (deferred)

### Progress Table (v39.0)

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 310. Session Trace ID | 1/2 | Plan 1 Complete | 2026-04-02 |

*Last updated: 2026-04-02 after MI-5 gap creation*

---

## v42.0 Meshed Intelligence Migration

**Goal:** Move the MI tier engine from rc-agent to rc-sentry so it can autonomously diagnose and heal rc-agent from the outside — eliminating the blind spot where rc-agent's death kills the entire self-healing system. Motivated by real incident: 2026-04-03 Pod 1+7 had rc-agent dead for hours with MI completely blind.

**Phases:** 4  |  **Coverage:** 14/14 requirements mapped

**Dependency graph:**
```
321 (External Monitoring & Alert Chain)
  └──> 322 (MI Core Engine Migration)
         └──> 323 (MMA Engine & Cognitive Gate Migration)
                └──> 324 (True Mesh Intelligence)
```

### Phases

- [x] **Phase 321: External Monitoring & Alert Chain** — MON-01, MON-02, MON-03, MON-04, MON-05 (completed 2026-04-06)
- [x] **Phase 322: MI Core Engine Migration** — MIG-01, MIG-02, MIG-03, MIG-05 (completed 2026-04-06)
- [x] **Phase 323: MMA Engine & Cognitive Gate Migration** — MIG-04, MIG-06 (completed 2026-04-07)
- [x] **Phase 324: True Mesh Intelligence** — MESH-01, MESH-02, MESH-03 (completed 2026-04-06)

---

### Phase 321: External Monitoring & Alert Chain
**Goal**: rc-sentry can observe rc-agent's health from the outside and alert staff/Bono when rc-agent is dead — independent of rc-agent's own API
**Depends on**: Nothing (foundation for v42.0)
**Requirements**: MON-01, MON-02, MON-03, MON-04, MON-05
**Success Criteria** (what must be TRUE):
  1. When rc-agent process is dead on a pod, rc-sentry detects it within 30 seconds via `tasklist` inspection and `:8090/health` polling — without calling any rc-agent API
  2. The server's pod_healer successfully recovers a pod when rc-agent :8090 is unreachable by falling back to rc-sentry :8091 — verified by killing rc-agent and watching the pod recover
  3. After 3 rc-agent restarts within 10 minutes, rc-sentry stops restarting (exponential backoff applies), clears the `MAINTENANCE_MODE` sentinel automatically after the backoff window, and sends a WhatsApp alert naming the pod and restart count
  4. When rc-agent dies on any pod, a WhatsApp message reaches Uday/staff naming the pod — COMMS_PSK is deployed to all 8 pods and the watchdog alert path is live
  5. rc-sentry can capture a pod screenshot and analyze pixel patterns to verify that the blanking screen is actually displayed (not just that the Edge process exists)
**Plans**: 3 plans
Plans:
- [x] 321-01-PLAN.md — Dual-detection watchdog + MON-02/MON-03 verification
- [x] 321-02-PLAN.md — Direct WhatsApp alert via Evolution API
- [x] 321-03-PLAN.md — Screenshot-based blanking verification

### Phase 322: MI Core Engine Migration
**Goal**: The tier engine, diagnostic engine, knowledge base, and telemetry proxy are running in rc-sentry — rc-agent continues working via a thin forwarding proxy during and after migration
**Depends on**: Phase 321 (monitoring foundation must be live before adding complex MI logic to sentry)
**Requirements**: MIG-01, MIG-02, MIG-03, MIG-05
**Success Criteria** (what must be TRUE):
  1. rc-sentry runs the 5-tier decision tree and produces a `TierDiagnosis` when triggered by a `DiagnosticTrigger` — independent of whether rc-agent is alive
  2. rc-sentry's diagnostic engine classifies anomalies and fires `DiagnosticTrigger` events through an internal std::sync channel (no tokio dependency) — observable via rc-sentry's :8091 `/debug` endpoint
  3. rc-sentry's knowledge base can query the SQLite solution DB and return a `SolutionRecord` for a given failure pattern — the KB is accessible even when rc-agent is dead
  4. rc-agent's MI modules are replaced with a thin proxy that forwards telemetry to rc-sentry :8091 via HTTP — rc-agent still compiles and all existing WS messages continue working
**Plans**: TBD

### Phase 323: MMA Engine & Cognitive Gate Migration
**Goal**: The MMA audit engine and cognitive gate planner run in rc-sentry — multi-model diagnosis is available even when rc-agent is fully dead
**Depends on**: Phase 322 (MMA engine consumes tier + diagnostic output that must already be in sentry)
**Requirements**: MIG-04, MIG-06
**Success Criteria** (what must be TRUE):
  1. rc-sentry can initiate an MMA audit via OpenRouter (reading COMMS_PSK/OPENROUTER_KEY from environment) and record the consensus finding in the knowledge base — without rc-agent running
  2. MMA budget tracker in rc-sentry enforces the $5/session cap and logs spend to a local file readable via :8091 `/mma/status` endpoint
  3. rc-sentry's cognitive gate evaluates a diagnosis and produces a structured fix plan (JSON array of actions with risk + rollback) — the plan is visible at :8091 `/gate/last-plan`
  4. Diagnosis planner in rc-sentry can generate a structured fix sequence for at least the 5 most common failure patterns (rc-agent crash, game stuck, MAINTENANCE_MODE, WS disconnect, blanking failure)
**Plans**: TBD

### Phase 324: True Mesh Intelligence
**Goal**: Pods can coordinate directly with each other without routing through the server — solutions discovered on one pod propagate to the fleet and multiplayer game sessions can self-coordinate
**Depends on**: Phase 323 (MI must be fully in rc-sentry before adding cross-pod communication)
**Requirements**: MESH-01, MESH-02, MESH-03
**Success Criteria** (what must be TRUE):
  1. Pod 1 can send a message directly to Pod 2 via rc-sentry's peer channel (UDP or TCP on a dedicated port) without the server involved — verified by killing the server and watching a direct pod-to-pod ping succeed
  2. When Pods 3 and 5 are in the same F1 25 multiplayer session, rc-sentry on both pods can coordinate a synchronized launch — both pods receive `LaunchGame` within 500ms of each other without server orchestration
  3. When rc-sentry on Pod 4 records a solution to a known failure pattern (e.g. "MAINTENANCE_MODE clear + restart"), that solution propagates to all other pods via direct gossip within 60 seconds — verified by checking each pod's :8091 `/kb/solutions` count
**Plans**: TBD

---

### Progress Table (v42.0)

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 321. External Monitoring & Alert Chain | 3/3 | Complete   | 2026-04-06 |
| 322. MI Core Engine Migration | 4/4 | Complete   | 2026-04-06 |
| 323. MMA Engine & Cognitive Gate Migration | 2/2 | Complete   | 2026-04-07 |
| 324. True Mesh Intelligence | 2/2 | Complete   | 2026-04-06 |

*Last updated: 2026-04-03 — roadmap created*

## v43.0 Self-Audit & Visual Regression System

**Goal:** James autonomously verifies all frontend pages before/after fixes using Playwright screenshots, with hook-enforced compliance and deploy script integration -- eliminating the "fix blind from code" failure mode.

**Phases:** 4  |  **Coverage:** 17/17 requirements mapped

**Dependency graph:**
```
325 (Page Crawler)
  ├──> 326 (Visual Regression Tests)
  ├──> 327 (Enforcement & Deploy Integration) ←── also depends on 326
  └──> 328 (AI Self-Audit)
```

### Phases

- [x] **Phase 325: Page Crawler** — CRAWL-01, CRAWL-02, CRAWL-03, CRAWL-04 (completed 2026-04-06)
- [x] **Phase 326: Visual Regression Tests** — VR-01, VR-02, VR-03, VR-04 (completed 2026-04-06)
- [x] **Phase 327: Enforcement & Deploy Integration** — HOOK-01, HOOK-02, HOOK-03, DEPLOY-01, DEPLOY-02, DEPLOY-03 (completed 2026-04-06)
- [x] **Phase 328: AI Self-Audit** — AUDIT-01, AUDIT-02, AUDIT-03, AUDIT-04 (completed 2026-04-06)

---

### Phase 325: Page Crawler
**Goal**: James can capture screenshots of every frontend page on demand, with proper authentication and structured output
**Depends on**: Nothing (first phase of v43.0)
**Requirements**: CRAWL-01, CRAWL-02, CRAWL-03, CRAWL-04
**Success Criteria** (what must be TRUE):
  1. Running the crawler produces a screenshot for every reachable page across web (:3200), admin (:3201), and kiosk (:3300)
  2. Crawler authenticates via saved Playwright storageState (staff PIN) -- no manual login needed per run
  3. Screenshots are saved to `tests/screenshots/{app}/{route}/{timestamp}.png` with consistent naming
  4. Crawler accepts flags to target a specific app or specific page instead of always doing a full crawl
**Plans**: 1 plan
**UI hint**: yes

Plans:
- [x] 325-01: Page crawler script with auth, full crawl, and selective targeting

### Phase 326: Visual Regression Tests
**Goal**: Frontend changes are automatically compared against known-good baselines, with dynamic content properly masked
**Depends on**: Phase 325
**Requirements**: VR-01, VR-02, VR-03, VR-04
**Success Criteria** (what must be TRUE):
  1. Critical pages have Playwright toHaveScreenshot() tests that fail when layout or styling changes unexpectedly
  2. Dynamic content (timestamps, counters, live metrics) is masked per-page so legitimate data changes do not trigger false failures
  3. Baseline screenshots are committed in git alongside test files and update via --update-snapshots
  4. Running the visual regression suite before and after a frontend fix produces a clear before/after comparison
**Plans**: 1 plan
**UI hint**: yes

Plans:
- [x] 326-01: Playwright visual regression tests with baselines, masking config, and before/after workflow

### Phase 327: Enforcement & Deploy Integration
**Goal**: Frontend completion claims require screenshot evidence, and deploys automatically detect visual regressions
**Depends on**: Phase 325, Phase 326
**Requirements**: HOOK-01, HOOK-02, HOOK-03, DEPLOY-01, DEPLOY-02, DEPLOY-03
**Success Criteria** (what must be TRUE):
  1. Claude Code hook blocks "fixed/done/resolved" claims for frontend changes unless a screenshot file newer than the last code edit exists in the session
  2. Hook only fires for frontend-related file changes (Next.js pages, CSS, React components) -- Rust backend and script changes are unaffected
  3. After deploy-nextjs.sh completes, the page crawler runs automatically and the deploy exits with failure if visual regressions are detected
  4. Deploy output includes a build hash verification table showing expected vs running build on server and cloud targets
**Plans**: 2 plans

Plans:
- [x] 327-01: Claude Code enforcement hook (screenshot evidence gate for frontend claims)
- [x] 327-02: Deploy script integration (auto-crawl after deploy, hash verification table, regression gate)

### Phase 328: AI Self-Audit
**Goal**: James autonomously identifies pages that look wrong by comparing live screenshots against documented expected behavior
**Depends on**: Phase 325
**Requirements**: AUDIT-01, AUDIT-02, AUDIT-03, AUDIT-04
**Success Criteria** (what must be TRUE):
  1. Every critical page has a description file documenting expected layout, data sources, and key interactions
  2. James can read a fresh screenshot via the Read tool and compare it against the page description to spot anomalies
  3. Running the self-audit produces an anomaly report listing pages that do not match expected behavior with specific discrepancies
  4. When James starts a session involving frontend work, the self-audit runs automatically to establish baseline awareness of current state
**Plans**: 2 plans

Plans:
- [x] 328-01: Page descriptions + self-audit script + anomaly report
- [x] 328-02: Session-start hook + CLAUDE.md self-audit standing rules

---

### Progress Table (v43.0)

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 325. Page Crawler | 1/1 | Complete    | 2026-04-06 |
| 326. Visual Regression Tests | 1/1 | Complete    | 2026-04-06 |
| 327. Enforcement & Deploy Integration | 2/2 | Complete    | 2026-04-06 |
| 328. AI Self-Audit | 2/2 | Complete    | 2026-04-06 |

*Last updated: 2026-04-06 -- roadmap created*

---

## v44.0 VMS Architecture Integration

**Goal:** Adopt 13 proven patterns from SRL VMS V5.0 — eliminate Edge browser dependency, unify restart authority, add multiplayer infrastructure, and close all operational gaps identified in the VMS gap analysis (2026-04-05).

**Phases:** 8  |  **Coverage:** 13 gaps mapped from systematic customer-journey elimination

**Why:** The Edge browser lock screen has caused 10+ bugs across 6 sessions (Stdio::null, session restore, startup boost, process counting, memory bloat, crash recovery). VMS uses native Win32 windows for blanking — zero browser bugs in 20+ years. Remaining 5 gaps are features VMS has that customers at competing venues expect.

**Dependency graph:**
```
329 (Native Win32 Blanking) ──> 330 (On-Track Display + Off-Track Blanking)
                                         │
331 (Process Architecture)               │
                                         v
332 (mDNS Discovery)           335 (Circuit Viewer)
         │
         v
333 (MP Local Server + Sync Lobby) ──> 334 (Follow-the-Server)
                                              │
                                     336 (Deploy Verification + E2E)
```

### Phases

- [ ] **Phase 329: Native Win32 Lock Screen** — WIN-01, WIN-02, WIN-03, WIN-04, WIN-05
- [ ] **Phase 330: Native On-Track Display + Off-Track Blanking** — OTD-01, OTD-02, OTD-03, OTD-04
- [ ] **Phase 331: Process Architecture Cleanup** — PROC-01, PROC-02, PROC-03
- [ ] **Phase 332: mDNS Auto-Discovery** — MDNS-01, MDNS-02, MDNS-03
- [ ] **Phase 333: MP Local Server + Sync Lobby** — MP-01, MP-02, MP-03, MP-04
- [ ] **Phase 334: Follow-the-Server Session Progression** — FTS-01, FTS-02, FTS-03
- [ ] **Phase 335: Live Circuit Viewer (Spectator)** — CIV-01, CIV-02, CIV-03
- [ ] **Phase 336: Deploy Verification & E2E Automation** — DVER-01, DVER-02, DVER-03

### Phase 329: Native Win32 Lock Screen
**Goal**: Replace Edge browser lock screen with a native Win32 window. Blanking, PIN entry, timer, session summary — all rendered via Win32 GDI+/Direct2D. Eliminates Edge dependency entirely.
**Depends on**: None (foundational)
**Requirements**: WIN-01, WIN-02, WIN-03, WIN-04, WIN-05
**Success Criteria** (what must be TRUE):
  1. Lock screen renders Racing Point branding (logo, red #E10600) on a native Win32 HWND spanning all monitors (7680x1440)
  2. Edge browser is NOT launched at any point during the lock screen lifecycle
  3. PIN entry works via native input handling (keyboard events, not DOM)
  4. Timer display updates every second during active billing session
  5. Session summary renders post-session stats (duration, laps, best lap)
  6. Window uses HWND_TOPMOST + SetWindowPos to cover all monitors (same as current Edge --app approach)
  7. Memory footprint < 20MB (vs Edge ~300MB)
  8. No Stdio::null issues — native window doesn't inherit console handles
**Plans**: 3 plans

Plans:
- [x] 329-01-PLAN.md — Win32 window foundation + font embedding + non-interactive state painters
- [x] 329-02-PLAN.md — PIN entry keyboard handling + QR code renderer + ActiveSession timer
- [ ] 329-03-PLAN.md — Remaining state painters + Edge code removal + Pod 8 visual verification

### Phase 330: Native On-Track Display + Off-Track Blanking
**Goal**: Replace Edge-based in-session overlay with native Win32 GDI+ rendering. Add VMS-style off-track blanking (screen shows branding when car goes off-track mid-session).
**Depends on**: Phase 329 (shares native rendering infrastructure)
**Requirements**: OTD-01, OTD-02, OTD-03, OTD-04
**Success Criteria** (what must be TRUE):
  1. In-session HUD (timer, lap count, position) renders as native Win32 overlay
  2. Off-track detection triggers via `isValidLap` transition in AC shared memory
  3. When car goes off-track, blanking screen appears within 500ms showing Racing Point branding
  4. When car returns to track, blanking hides within 500ms
  5. Off-track blanking is configurable (enable/disable per session type)
**Plans**: TBD

### Phase 331: Process Architecture Cleanup
**Goal**: Adopt VMS patterns — single restart authority (eliminate competing watchdog/sentry/schtask), remove binary rename (rollback_manager), fix Stdio::null at the root.
**Depends on**: None
**Requirements**: PROC-01, PROC-02, PROC-03
**Success Criteria** (what must be TRUE):
  1. Only ONE restart mechanism active: RCWatchdog service (SCM-style). Schtask and sentry restart paths removed.
  2. rollback_manager binary rename disabled — single binary, no prev/failed rename
  3. All Command::new() calls in rc-agent go through a single `spawn_safe()` helper that sets Stdio::null + appropriate creation_flags
  4. Agent survives 24h uptime through multiple game launch/stop cycles with zero restarts
**Plans:** 1/3 plans executed

Plans:
- [ ] 331-01-PLAN.md — Create spawn_safe() helper in rc-common and migrate all 76 Command::new call sites
- [x] 331-02-PLAN.md — Single restart authority: remove binary rename, clean sentry dead code

### Phase 332: mDNS Auto-Discovery
**Goal**: Pods auto-discover the racecontrol server via mDNS (`_racecontrol._tcp.local.`) instead of hardcoded IP in TOML config. Enables zero-config pod setup.
**Depends on**: None
**Requirements**: MDNS-01, MDNS-02, MDNS-03
**Success Criteria** (what must be TRUE):
  1. Server broadcasts `_racecontrol._tcp.local.` via mDNS on startup
  2. Pod agent discovers server without `[core] url` in TOML (falls back to TOML if mDNS fails)
  3. Pod reconnects via mDNS if server IP changes (DHCP environment)
**Plans**: TBD

### Phase 333: MP Local Server + Sync Lobby
**Goal**: Multiplayer sessions run a local AC dedicated server (like VMS SimLauncher) instead of Content Manager URI hack. Add synchronized lobby — all pods enter simultaneously, hold until ready, start in sync.
**Depends on**: None (but benefits from Phase 332 for discovery)
**Requirements**: MP-01, MP-02, MP-03, MP-04
**Success Criteria** (what must be TRUE):
  1. MP launch starts a local `acServer.exe` with generated `server_cfg.ini` + `entry_list.ini`
  2. Pod clients connect to local server automatically (no CM URI)
  3. Lobby holds until all assigned pods are connected (120s timeout, proceed-anyway)
  4. Race start is synchronized across all pods (server controls session start)
  5. If a pod disconnects mid-race, remaining pods continue (no full restart)
**Plans**: TBD

### Phase 334: Follow-the-Server Session Progression
**Goal**: VMS-style automatic session progression — server cycles Practice → Qualifying → Race, kiosk binds to running server, pods auto-join as server advances.
**Depends on**: Phase 333 (local server infrastructure)
**Requirements**: FTS-01, FTS-02, FTS-03
**Success Criteria** (what must be TRUE):
  1. Staff configures a race weekend (practice + quali + race) in one action
  2. Server automatically progresses through sessions (time-based or manual trigger)
  3. Pods that join mid-weekend enter the current session (not restart from practice)
  4. Session transition is visible on spectator display and dashboard
**Plans**: TBD

### Phase 335: Live Circuit Viewer (Spectator)
**Goal**: Real-time car positions on a track map for lobby TVs / spectator displays. SVG-based rendering using normalized spline position from AC shared memory.
**Depends on**: Phase 330 (off-track detection provides car position data)
**Requirements**: CIV-01, CIV-02, CIV-03
**Success Criteria** (what must be TRUE):
  1. Web page at `/spectator/circuit` shows track outline with live car dots
  2. Car positions update at 10Hz from AC telemetry via WebSocket
  3. Works for all installed tracks (SVG generated from AC track data)
  4. Displays on spectator TV (192.168.31.200) via Edge kiosk
**Plans**: TBD

### Phase 336: Deploy Verification & E2E Automation
**Goal**: Automated post-deploy verification that checks blanking, game launch, and billing E2E — not just build_id. Plus automated deploy parity enforcement across all targets.
**Depends on**: Phase 329 (native blanking changes verification approach)
**Requirements**: DVER-01, DVER-02, DVER-03
**Success Criteria** (what must be TRUE):
  1. Post-deploy script checks: blanking active (edge/win32 count > 0), screenshot non-black, build_id matches, WS connected, Session 1
  2. Deploy parity enforced: script checks Server + all 8 pods + POS + Cloud + comms-link builds and warns on drift
  3. E2E test script: creates test billing session, launches game, verifies AC process alive + screenshot shows game, stops session, verifies refund — all automated
**Plans**: TBD

### Progress Table (v44.0)

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 329. Native Win32 Lock Screen | 1/3 | In Progress|  |
| 330. On-Track Display + Off-Track Blanking | 0/TBD | Not Started | — |
| 331. Process Architecture Cleanup | 1/2 | In Progress | 331-02 done |
| 332. mDNS Auto-Discovery | 0/TBD | Not Started | — |
| 333. MP Local Server + Sync Lobby | 0/TBD | Not Started | — |
| 334. Follow-the-Server | 0/TBD | Not Started | — |
| 335. Live Circuit Viewer | 0/TBD | Not Started | — |
| 336. Deploy Verification & E2E | 0/TBD | Not Started | — |

*Created: 2026-04-07 — from VMS gap analysis (13 items, systematic customer-journey elimination)*

---

## v45.0 Credits/Rupees Wallet Separation

**Goal:** Separate wallet tracking into rupee deposits (real money, for balance sheet) and credits (what customers see and spend). Bonuses are promotional credits. Cash refunds only refund deposited rupees, never bonus credits. Unified API contract consumed by admin portal, POS, and kiosk.

**Business Rules (confirmed 2026-04-07):**
1. Customer deposits rupees → converted to credits 1:1
2. Bonus/promotion credits added on top (not real money)
3. Customer only sees and spends **credits**
4. Session charges, cafe, merchandise — all in credits
5. Game reset → credits refunded automatically
6. Cash refund (rupees back) → **admin-only**, max = deposited rupees - spent - already refunded. Bonus forfeit.

**Depends on:** None (can run in parallel with v44.0)

```
Phase Dependency Graph:

  337 (DB Schema)
    ↓
  338 (Wallet Rust + Accounting)
    ↓
  339 (API Endpoints)
    ↓
  340 (Admin Dashboard — local + cloud)  ←  341 (POS + Kiosk display)
    ↓
  342 (Cloud Sync + Deploy + E2E Verify)
```

### Phases

- [x] **Phase 337: DB Schema Migration** — WAL-01: Add rupee_deposited_paise, rupee_refunded_paise, bonus_credited_paise to wallets. Add currency_type to wallet_transactions. Backfill from existing txn_type. (completed 2026-04-07)
- [x] **Phase 338: Wallet Core Logic** — WAL-02: Update credit/debit in wallet.rs. Bonus credits go to bonus_credited_paise. Top-ups go to rupee_deposited_paise. Debit burns from single balance_paise pool. Cash refund capped at net rupee deposits. (completed 2026-04-07)
- [ ] **Phase 339: API Endpoints** — WAL-03: Update GET /wallet, POST /topup, POST /refund responses. New fields: balance_credits, rupee_deposited, rupee_refunded, bonus_credited, max_cash_refund. Same contract for admin/POS/kiosk.
- [ ] **Phase 340: Admin Dashboard** — WAL-04: Add credit management panel to billing/reports and billing/history. Show rupee deposits vs bonus credits. Cash refund button with max-refundable calculation. Deploy to BOTH local (.23:3201) and cloud.
- [ ] **Phase 341: POS + Kiosk Display** — WAL-05: Fix ₹ symbol on drivers page → "credits". Verify POS billing page shows credits. Kiosk pricing shows credits (already correct). Ensure unified API contract.
- [ ] **Phase 342: Cloud Sync + E2E Verify** — WAL-06: Update cloud_sync.rs push/pull for new columns. Update process_debit_intents. E2E test: topup → bonus → spend → verify balances → cash refund → verify max cap.

### Phase 337: DB Schema Migration
**Goal**: Add wallet tracking columns for rupee/credit separation without breaking existing functionality.
**Success Criteria:**
  1. `wallets` table has `rupee_deposited_paise`, `rupee_refunded_paise`, `bonus_credited_paise` columns
  2. `wallet_transactions` table has `currency_type` column ('rupee' or 'credit')
  3. Existing transactions backfilled: topup_* = 'rupee', bonus/adjustment = 'credit'
  4. `balance_paise` unchanged — still the single spendable credits pool
  5. Migration is idempotent (ALTER TABLE IF NOT EXISTS pattern)
  6. Cloud DB also gets the migration on next sync
**Plans:** 1/1 plans complete
Plans:
- [x] 337-01-PLAN.md — Add ALTER TABLE columns + backfill queries to migrate()

### Phase 338: Wallet Core Logic
**Goal**: Update wallet.rs so top-ups track rupee deposits, bonuses track bonus credits, and cash refunds are capped.
**Success Criteria:**
  1. `credit_in_tx` increments `rupee_deposited_paise` for topup_* txn_types
  2. `credit_in_tx` increments `bonus_credited_paise` for bonus/adjustment txn_types
  3. Cash refund (`refund_wallet`) capped at `rupee_deposited_paise - rupee_refunded_paise - total_debited_paise` (floor 0)
  4. Cash refund increments `rupee_refunded_paise`
  5. Credit refund (game reset) only touches `balance_paise` — no rupee tracking
  6. Accounting journal: cash refund → Dr. acc_wallet Cr. acc_cash/bank; credit refund → Dr. acc_wallet Cr. acc_refunds
**Plans:** 2/2 plans complete
Plans:
- [x] 338-01-PLAN.md — Update structs + credit/debit functions with rupee/bonus tracking
- [x] 338-02-PLAN.md — Add cash_refund + get_max_cash_refund + accounting journal

### Phase 339: API Endpoints
**Goal**: Unified wallet API response consumed by admin, POS, and kiosk.
**Success Criteria:**
  1. GET /wallet/{driver_id} returns: `{ balance_credits, rupee_deposited, rupee_refunded, bonus_credited, max_cash_refund, total_spent, transactions_count }`
  2. POST /wallet/{driver_id}/topup response includes: `{ new_balance_credits, bonus_credits_granted, rupee_amount }`
  3. POST /wallet/{driver_id}/refund differentiates: `{ type: "credit_refund" | "cash_refund", amount, max_allowed }`
  4. GET /wallet/transactions includes `currency_type` per transaction
  5. Same response schema served on all ports (8080 API) — no per-frontend variants

### Phase 340: Admin Dashboard
**Goal**: Add credit/rupee management UI to admin portal, deployed locally AND on cloud.
**Success Criteria:**
  1. `/billing/reports` page shows: total rupee deposits, total bonus credits issued, total credits spent, total cash refunds
  2. `/billing/history` page shows per-transaction `currency_type` badge (rupee/credit)
  3. Cash refund button shows max refundable amount and requires confirmation
  4. Credit adjustment button (admin adds/removes credits manually with reason)
  5. Dashboard accessible at `192.168.31.23:3201/billing/reports` AND `racingpoint.cloud:3201/billing/reports`

### Phase 341: POS + Kiosk Display
**Goal**: All customer-facing displays show "credits", never "₹".
**Success Criteria:**
  1. `web/src/app/drivers/page.tsx` shows "credits" not "₹"
  2. POS billing page (`192.168.31.130:3200/billing`) shows credits
  3. Kiosk pricing shows credits (verify already correct)
  4. PWA wallet shows credits (verify already correct)

### Phase 342: Cloud Sync + E2E Verify
**Goal**: Cloud sync pushes/pulls new wallet columns. Full E2E test of the financial flow.
**Success Criteria:**
  1. `cloud_sync.rs` push includes `rupee_deposited_paise`, `rupee_refunded_paise`, `bonus_credited_paise`
  2. `cloud_sync.rs` upsert_wallet handles new columns
  3. `process_debit_intents` works with new schema (still debits from `balance_paise`)
  4. E2E test: topup ₹1000 → verify 1000 credits + bonus → spend 200 → verify balance → request cash refund → verify max = ₹800 (not ₹800 + bonus)

### Progress Table (v45.0)

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 337. DB Schema Migration | 1/1 | Complete   | 2026-04-07 |
| 338. Wallet Core Logic | 2/2 | Complete   | 2026-04-07 |
| 339. API Endpoints | 0/TBD | Not Started | — |
| 340. Admin Dashboard | 0/TBD | Not Started | — |
| 341. POS + Kiosk Display | 0/TBD | Not Started | — |
| 342. Cloud Sync + E2E | 0/TBD | Not Started | — |

*Created: 2026-04-07 — business rules confirmed with Uday. See memory: project_credits_rupees_separation.md*
