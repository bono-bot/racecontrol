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
- [x] **v44.0 VMS Architecture Integration** — Phases 329-336 (8 phases, all code complete 2026-04-08)
- [x] **v45.0 Credits/Rupees Wallet Separation** — Phases 337-342 (6 phases, shipped 2026-04-07)
- [ ] **v46.0 Game Launch Diagnostics** — Phases 361-367 (Phase 362 shipped 2026-04-09 `a9b5eaa3`; Phase 363 code-complete + tested 2026-04-10, deploy+MMA deferred)
- [ ] **v47.0 Admin Dashboard Venue-Ready Hardening** — Phases 344-360 (17 phases, expanded 2026-04-09 after SSOT gap audit)
- [x] **v48.0 Codebase Architecture — Department-Driven Event Mesh** — Phases 369-382 (14 phases, 10 code-committed 2026-04-13, deploy pending)
- [ ] **v49.0 Unified RaceControl Operations** — Phases 383-392 (10 phases, defined 2026-04-14)

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

- [x] **Phase 329: Native Win32 Lock Screen** — WIN-01, WIN-02, WIN-03, WIN-04, WIN-05 (completed 2026-04-08)
- [x] **Phase 330: Native On-Track Display + Off-Track Blanking** — OTD-01, OTD-02, OTD-03, OTD-04 (completed 2026-04-08)
- [x] **Phase 331: Process Architecture Cleanup** — PROC-01, PROC-02, PROC-03 (completed 2026-04-08)
- [x] **Phase 332: mDNS Auto-Discovery** — MDNS-01, MDNS-02, MDNS-03 (completed 2026-04-08)
- [x] **Phase 333: MP Local Server + Sync Lobby** — MP-01, MP-02, MP-03, MP-04 (completed 2026-04-08)
- [x] **Phase 334: Follow-the-Server Session Progression** — FTS-01, FTS-02, FTS-03 (completed 2026-04-08)
- [x] **Phase 335: Live Circuit Viewer (Spectator)** — CIV-01, CIV-02, CIV-03 (completed 2026-04-08)
- [x] **Phase 336: Deploy Verification & E2E Automation** — DVER-01, DVER-02, DVER-03 (completed 2026-04-08)

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
- [x] 329-03-PLAN.md — MMA blanking health check updated to native Win32 window (74035454)

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
- [x] 331-01-PLAN.md — Migrated remaining 20 Command::new sites, fixed 3 Stdio::null bugs (793d6d04)
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
| 329. Native Win32 Lock Screen | 3/3 | Complete | 2026-04-08 |
| 330. On-Track Display + Off-Track Blanking | 1/1 | Complete | 2026-04-08 |
| 331. Process Architecture Cleanup | 2/2 | Complete | 2026-04-08 |
| 332. mDNS Auto-Discovery | 1/1 | Complete | 2026-04-08 |
| 333. MP Local Server + Sync Lobby | 1/1 | Complete | 2026-04-08 |
| 334. Follow-the-Server | 1/1 | Complete | 2026-04-08 |
| 335. Live Circuit Viewer | 1/1 | Complete | 2026-04-08 |
| 336. Deploy Verification & E2E | 1/1 | Complete | 2026-04-08 |

*Created: 2026-04-07. All phases completed 2026-04-08 (autonomous overnight execution).*

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
- [x] **Phase 339: API Endpoints** — WAL-03: Update GET /wallet, POST /topup, POST /refund responses. New fields: balance_credits, rupee_deposited, rupee_refunded, bonus_credited, max_cash_refund. Same contract for admin/POS/kiosk. (completed 2026-04-07)
- [x] **Phase 340: Admin Dashboard** — WAL-04: Add credit management panel to billing/reports and billing/history. Show rupee deposits vs bonus credits. Cash refund button with max-refundable calculation. Deploy to BOTH local (.23:3201) and cloud. (completed 2026-04-07)
- [x] **Phase 341: POS + Kiosk Display** — WAL-05: Fix ₹ symbol on drivers page → "credits". Verify POS billing page shows credits. Kiosk pricing shows credits (already correct). Ensure unified API contract. (completed 2026-04-07)
- [x] **Phase 342: Cloud Sync + E2E Verify** — WAL-06: Update cloud_sync.rs push/pull for new columns. Update process_debit_intents. E2E test: topup → bonus → spend → verify balances → cash refund → verify max cap. (completed 2026-04-07)

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
  3. POST /wallet/{driver_id}/refund returns `{ type: "credit_refund", new_balance_credits, max_cash_refund }`; POST /wallet/{driver_id}/cash-refund returns `{ type: "cash_refund", amount, new_balance_credits, max_cash_refund_remaining }`
  4. GET /wallet/transactions includes `currency_type` per transaction
  5. Same response schema served on all ports (8080 API) — no per-frontend variants
**Plans:** 2/2 plans complete
Plans:
- [x] 339-01-PLAN.md — WalletInfo serde renames + handler response field updates (topup, webhook, transactions)
- [x] 339-02-PLAN.md — Cash refund endpoint + credit refund type differentiation

### Phase 340: Admin Dashboard
**Goal**: Add credit/rupee management UI to admin portal, deployed locally AND on cloud.
**Success Criteria:**
  1. `/billing/reports` page shows: total rupee deposits, total bonus credits issued, total credits spent, total cash refunds
  2. `/billing/history` page shows per-transaction `currency_type` badge (rupee/credit)
  3. Cash refund button shows max refundable amount and requires confirmation
  4. Credit adjustment button (admin adds/removes credits manually with reason)
  5. Dashboard accessible at `192.168.31.23:3201/billing/reports` AND `racingpoint.cloud:3201/billing/reports`
**Plans:** 3/3 plans complete
Plans:
- [x] 340-01-PLAN.md — Wallet API module + reports summary cards + currency_type badges
- [x] 340-02-PLAN.md — Cash refund button + credit adjustment button
- [x] 340-03-PLAN.md — Build and deploy to local + cloud

### Phase 341: POS + Kiosk Display
**Goal**: All customer-facing displays show "credits", never "₹".
**Success Criteria:**
  1. `web/src/app/drivers/page.tsx` shows "credits" not "₹"
  2. POS billing page (`192.168.31.130:3200/billing`) shows credits
  3. Kiosk pricing shows credits (verify already correct)
  4. PWA wallet shows credits (verify already correct)
**Plans:** 1/1 plans complete
Plans:
- [x] 341-01-PLAN.md — Fix drivers page rupee to credits + verify POS/kiosk/PWA displays

### Phase 342: Cloud Sync + E2E Verify
**Goal**: Cloud sync pushes/pulls new wallet columns. Full E2E test of the financial flow.
**Success Criteria:**
  1. `cloud_sync.rs` push includes `rupee_deposited_paise`, `rupee_refunded_paise`, `bonus_credited_paise`
  2. `cloud_sync.rs` upsert_wallet handles new columns
  3. `process_debit_intents` works with new schema (still debits from `balance_paise`)
  4. E2E test: topup ₹1000 → verify 1000 credits + bonus → spend 200 → verify balance → request cash refund → verify max = ₹800 (not ₹800 + bonus)
**Plans:** 1/1 plans complete
Plans:
- [x] 342-01-PLAN.md — Cloud sync push/pull new wallet columns + E2E test checklist

### Progress Table (v45.0)

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 337. DB Schema Migration | 1/1 | Complete   | 2026-04-07 |
| 338. Wallet Core Logic | 2/2 | Complete   | 2026-04-07 |
| 339. API Endpoints | 2/2 | Complete    | 2026-04-07 |
| 340. Admin Dashboard | 3/3 | Complete    | 2026-04-07 |
| 341. POS + Kiosk Display | 0/TBD | Complete    | 2026-04-07 |
| 342. Cloud Sync + E2E | 1/1 | Complete    | 2026-04-07 |

*Created: 2026-04-07 — business rules confirmed with Uday. See memory: project_credits_rupees_separation.md*

---

## Backlog

### Phase 999.1: rc-agent forza test Instant underflow on fresh CI VMs (BACKLOG)

**Goal:** [Captured for future planning]
**Requirements:** TBD
**Plans:** 0 plans

**Bug summary:** `crates/rc-agent/src/session_enforcer.rs:296` does `let past = Instant::now() - Duration::from_secs(3600);` which panics with "overflow when subtracting duration from instant" on windows-latest CI runners whose boot uptime is less than 1 hour. James local passes (high uptime). Latent bug masked for weeks/months by the rc-sentry-ai LNK1120 build failure that prevented CI from ever reaching the `Test rc-agent` step. Surfaced 2026-04-09 after `d027332a` fixed the rc-sentry-ai build.

**Evidence:** CI runs 24183092938 (commit d027332a) and 24183396002 (commit d5b8af2f) both fail at `session_enforcer::tests::test_tick_terminate_forza_motorsport` with identical 763 passed / 1 failed counts. Stack: `std/src/time.rs:445:33`. Other 5 tests in the same file use 59-200s durations and are safe.

**Fix options:**
1. Refactor `SessionEnforcer::new_with_start` to take `Duration` (time-since-start) instead of raw `Instant` — cleanest, touches production API, ~10 lines.
2. Use `Instant::now().checked_sub(Duration::from_secs(3600)).unwrap_or_else(Instant::now)` — semantically wrong (test would see 0 elapsed, not 3600).
3. Reduce the test duration from 3600s to 60s with adjusted session-duration thresholds — smallest change, slightly weakens test coverage.
4. Add `#[cfg(test)]` mock time source — most flexible, most work.

**Recommendation:** Option 1. Scope: 1 file, ~10 lines, low risk.

Plans:
- [ ] TBD (promote with /gsd:review-backlog when ready)

### Phase 999.2: Deploy script must enforce target liveness before claiming complete (BACKLOG)

**Goal:** [Captured for future planning]
**Requirements:** TBD
**Plans:** 0 plans

**Motivation:** During the 2026-04-11 ecosystem CLD, Pod 1 was found running stale build `5f92578e` while Pods 2-8 run `b36e8a7b` (uniform). Root cause per LOGBOOK: Pod 1 was powered off during the v46.0 fleet deploy, so `deploy-all-pods.sh` skipped it silently and the deploy was reported "complete." No alert, no retry queue, no visible drift until a full fleet health probe ran hours later. This is a deploy-safety gap, not a pod-ops gap: the script's exit code said SUCCESS while one target was untouched. Every future fleet deploy has the same blind spot for any pod that happens to be off/unreachable at deploy time.

**Scope** (in `scripts/deploy/`):
1. **Pre-flight liveness check** — Before beginning any fleet deploy (`deploy-all-pods.sh`, `deploy-all.sh`, any script that loops over Pods 1-8 / POS / server), ping each target (rc-agent `/health` or TCP probe on :8090) and fail-fast if any target is unreachable. Exit non-zero with an explicit "Target X unreachable — aborting fleet deploy" message.
2. **Post-deploy verification loop** — After the deploy step completes on each target, verify via `/health` that the new `build_id` matches the expected hash. If any target still reports the old hash, retry up to N times, then fail the overall deploy with a drift report.
3. **Drift report artifact** — On any partial-deploy failure, write `deploy-staging/drift-report-<timestamp>.json` listing which targets succeeded, which failed, and the delta hashes. This becomes the resume input for a follow-on deploy.
4. **Override flag** — `--allow-unreachable=<pod1,pod3>` so operator can explicitly acknowledge a known-offline target (e.g. pod being serviced) without bypassing the check globally.

**Out of scope:**
- Fixing Pod 1 itself (operational one-off — handle in next live deploy window, not this backlog).
- Rewriting `deploy-all-pods.sh` architecture.
- Cloud/Bono VPS deploy parallel (separate backlog if needed).

**Permanence:** Source-code change to deploy scripts. Survives redeploy. No manual server-side state.

**Evidence (CLD 2026-04-11):**
- Pod 1 .89 `/health`: `build_id=5f92578e`, uptime 4537s
- Pods 2-8 `/health`: `build_id=b36e8a7b` uniform, uptime ~15056s
- LOGBOOK 2026-04-11 14:30 IST: "Pod 1 powered off during v46.0 deploy"
- Current `deploy-all-pods.sh`: no pre-flight, no post-verify, exit 0 on silent skip

**Priority:** Medium — not actively breaking prod (Pod 1 still runs a working older build), but every future deploy has the same blind spot. Should be done before the next major fleet rollout.

Plans:
- [ ] TBD (promote with /gsd:review-backlog when ready)

### Phase 999.3: Port drift-proof deploy-audit walker to web/ and kiosk/ (BACKLOG)

**Goal:** [Captured for future planning]
**Requirements:** TBD
**Plans:** 0 plans

**Motivation:** Sibling to `racingpoint-admin` backlog 999.1 ("Drift-proof deploy-audit — auto-derive pages_expected from filesystem walker"). The admin walker is the reference implementation; web/ and kiosk/ in this repo have the same drift bug and need the same fix. The 2026-04-11 CLD measured the drift directly:

| App | Repo | `/api/health` endpoint | pages_expected | pages_available | Delta |
|---|---|---|---|---|---|
| Admin | racingpoint-admin | http://192.168.31.23:3201/api/health | 32 | 52 | +20 |
| **Web** | **racecontrol/web** | http://192.168.31.23:3200/api/health | **25** | **49** | **+22** |
| **Kiosk** | **racecontrol/kiosk** | http://192.168.31.23:3300/kiosk/api/health | **9** | **16** | **+5** |

Web and kiosk are both reporting `healthy: true` while their hardcoded `pages_expected` arrays are 47 entries short of reality (22 + 5 + the admin repo's 20, totalled across all three apps). The health check is cosmetic on all three Next.js apps.

**Scope (in this repo only — admin walker lives in `racingpoint-admin` 999.1):**

1. **Port the walker to `web/`** — Implement the same filesystem walker in `web/src/app/api/health/route.ts` (or equivalent). Walk `web/src/app/**/page.tsx` with App-Router rules (group routes `(...)`, dynamic segments `[id]`, catch-alls `[...slug]`, route handlers vs pages). Return `pages_expected_count` + `pages_expected_hash` + `pages_available`.
2. **Port the walker to `kiosk/`** — Same pattern in `kiosk/src/app/api/health/route.ts`. Kiosk uses `basePath: /kiosk` so the health endpoint is at `/kiosk/api/health` — verify the walker runs against the correct source tree, not the deployed bundle.
3. **Drift-as-error** — `pages_available != pages_expected_count` flips `healthy: false` with `drift_detected: true` reason. "More is fine" is not fine — adds should go through code review, not silent health-passes.
4. **Consumer wiring in `scripts/deploy/deploy-audit.sh`** — Read `pages_expected_hash` + `pages_expected_count` from all three Next.js apps (admin .23:3201, web .23:3200, kiosk .23:3300) via `/api/health`, and fail the post-deploy gate if any app reports drift. This closes the Deploy Manifest Protocol loop for frontends per Standing Rule DMP-01.

**Dependency:** `racingpoint-admin` 999.1 (reference implementation — should land first so web+kiosk can mirror the exact pattern, including the `pages_expected_hash` algorithm, output format, and error-flipping semantics).

**Out of scope:**
- Fixing the 47 missing entries by hand (band-aid, not the fix — the walker makes the list authoritative).
- API route health (separate concern from page inventory).
- PMP-01 cross-machine parity (that gate lives in comms-link backlog 999.1 and activates automatically once this lands).
- Rust racecontrol `/api/v1/health` — that's Axum, different code path, separate concern.

**Permanence:** Source-code change (Next.js API route + deploy script consumer). Survives redeploy. No manual state.

**Priority:** Medium-High — paired with admin 999.1. Two of the three broken health checks live in this repo. Any DMP frontend-gate claim is false until this lands alongside the admin walker.

**Cross-reference:**
- Parent: `racingpoint-admin` .planning/phases/999.1-drift-proof-deploy-audit-walker/
- Consumer: `comms-link` .planning/phases/999.1-page-manifest-parity-check/ (PMP-01 — activates once walkers ship)

Plans:
- [ ] TBD (promote with /gsd:review-backlog when ready)

---

## v47.0 Admin Dashboard Venue-Ready Hardening

**Goal:** Make the admin dashboard (venue .23:3201 + cloud admin.racingpoint.cloud) a resilient, venue-ready central source of truth before customers start using the venue. Close 18 audit findings from the 2026-04-09 Vishal-PIN incident and absorb the superseded Phase 343 Plan 03 (admin PIN UI).

**Phases:** 12 | **Requirements:** 63 mapped | **Waves:** 4

**Hard dependency:** Phase 343 Plans 01+02+04 (racecontrol backend) must ship before Phase 347 (Admin Staff Management UI). Other phases unblocked.

### Phase Summary Table

| # | Phase | Wave | Goal | Requirements | Depends on |
|---|---|---|---|---|---|
| 344 | Unbreakable Deploys | 1 | Single admin-deploy.sh + verify gate + rollback + Node pin | ADMIN-01..07 | — |
| 345 | Backend Resilience | 1 | Module-load errors never 500, admin.db lazy-load, remove hardcoded JWT/webhook defaults | ADMIN-08..13 | 344 |
| 346 | Cafe Menu Proxy Rewrite | 1 | Admin cafe/menu proxies to rc; drop dead admin.db tables; identity consolidation | ADMIN-14..20 | 345 |
| 347 | Admin Staff Management | 3 | /admin/staff page + change_staff_pin_safe + sync/pull-now | STAFF-01..10, DEP-01..04 | 344, 345, Phase 343 (racecontrol) |
| 348 | Auth Resilience | 2 | Per-staff-id lockout, DB persist, 12h JWT, break-glass | AUTH-01..07 | 344, 345 |
| 349 | DB Sync via Google Drive | 2 | Venue-to-cloud DB sync via shared Google Drive folder (revised from Litestream+B2) | SYNC-01..08 | 344 |
| 350 | Contract Tests | 3 | Playwright admin-to-POS/kiosk propagation + 46-page smoke | TEST-01..06 | 346, 347 |
| 351 | Data Durability | 4 | Daily backups + 30d retention + restore drill | OPS-08..14 | 349 |
| 352 | Health + WhatsApp Alerts | 2 | Per-subsystem /api/health probes + comms-link alerter | OPS-01..07 | 345 |
| 353 | Runbook + Training | 4 | Printed one-pagers at POS + incident log | OPS-15..19 | 347, 346 |
| 354 | UI Hardening | 2 | Hide dead routes + loading/empty/error states | UI-01..07 | 345 |
| 355 | Venue-Ready Readiness Review | 4 | Execute 18-criterion checklist + VERIFICATION.md | (all) | all above |

### Phase 344: Unbreakable Deploys

**Goal:** Replace the ad-hoc admin deploy procedure with a single scripted, verifiable, rollback-capable pipeline working identically on venue Windows + cloud Linux.

**Requirements:** ADMIN-01, ADMIN-02, ADMIN-03, ADMIN-04, ADMIN-05, ADMIN-06, ADMIN-07

**Success criteria:**
1. Fresh VM deploy completes in <3 minutes from clone to running admin
2. Post-deploy verify gate catches all 4 known P0 failure modes (missing static assets, missing env vars, ABI mismatch, login round-trip)
3. Rollback command reverts to previous build within 60 seconds
4. Deploy script fails loudly on any step error — no silent green deploys
5. Six stale `deploy-staging/set-*pin*` scripts deleted from git

**Plans:**
- [x] 344-01-PLAN — `admin-deploy.sh` + `verify-deploy.js` + `server-bootstrap.js` (`racingpoint-admin@b10b487`)
- [x] 344-02-PLAN — Node version pin + .nvmrc + engines + npm deploy scripts (`racingpoint-admin@b10b487`)
- [x] 344-03-PLAN — Archive 8 stale PIN scripts to `deploy-staging/archived/`

### Phase 345: Backend Resilience

**Goal:** No module-load error crashes a route. Every admin API returns structured JSON. Remove dangerous hardcoded defaults (JWT secret, webhook secret) from racecontrol.

**Requirements:** ADMIN-08, ADMIN-09, ADMIN-10, ADMIN-11, ADMIN-12, ADMIN-13

**Success criteria:**
1. Killing racecontrol — admin UI loads, shows degraded banner, admin-native pages still work
2. Every admin API error response is valid JSON with `error_code` field
3. Booting admin with missing `RC_URL` returns 503 on every rcFetch call — NOT a 500 at module load
4. Racecontrol refuses to start if `RC_JWT_SECRET` env is unset or default literal
5. Racecontrol refuses to start if `payment_webhook_secret` is unset

**Plans:**
- [ ] 345-01-PLAN — Admin rc proxy env validation moved inside handlers
- [ ] 345-02-PLAN — admin.db lazy-load + ABI auto-rebuild retry
- [ ] 345-03-PLAN — racecontrol halt-on-missing-secrets (C5 + C6)

### Phase 346: Cafe Menu Proxy Rewrite (SSOT)

**Goal:** Kill the dead-end `admin.db.menu_items` table. Admin cafe menu/inventory proxy to racecontrol `cafe_items`. Drop dead `admin.db.employees` table. Consolidate identity source reads.

**Requirements:** ADMIN-14, ADMIN-15, ADMIN-16, ADMIN-17, ADMIN-18, ADMIN-19, ADMIN-20

**Success criteria:**
1. Adding a menu item in admin appears on POS `/billing` within 10 seconds
2. `admin.db.menu_items`, `admin.db.inventory`, `admin.db.employees` tables no longer exist after migration
3. Startup schema-guard aborts boot if dropped tables re-appear
4. Schema-diff doc in PLAN.md matches all fields 1:1 between admin UI and racecontrol cafe_items
5. Pre-cutover snapshot of admin.db stored off-machine

**Plans:**
- [ ] 346-01-PLAN — Schema-diff + admin cafe routes rewrite to rcFetch
- [ ] 346-02-PLAN — Drop migration + schema-guard
- [ ] 346-03-PLAN — Identity source consolidation (C8) + terminal_pin cleanup (D6)

### Phase 347: Admin Staff Management

**Goal:** `/admin/staff` page + `change_staff_pin_safe` endpoint + `sync/pull-now` endpoint. Make safe PIN changes the easy path. Replaces curl/sqlite3/deploy-staging scripts.

**Requirements:** STAFF-01..10, DEP-01..04

**Success criteria:**
1. Uday can change a staff PIN via `/admin/staff` and see green "Verified on cloud + venue" within 5 seconds
2. Response includes both `cloud_verified` and `venue_verified` booleans
3. Kiosk on any pod accepts the new PIN within 10 seconds of green success
4. Old PIN no longer works on any pod or cloud admin
5. Feature flag `FEATURE_STAFF_PIN_UI` defaults off; pre-deploy script checks Phase 343 shipped
6. No plaintext PINs displayed anywhere in the UI

**Depends on:** Phase 343 Plans 01+02 must be SHIPPED in racecontrol (not just committed).

**Plans:**
3/3 plans complete
- [x] 347-02-PLAN — admin `/admin/staff` page + change-pin modal + Next.js proxy route
- [x] 347-03-PLAN — Feature flag + pre-deploy gate + smoke test

### Phase 348: Auth Resilience

**Goal:** Lockout survives restart. Per-staff-id tracking in addition to per-IP. 12h JWT with sliding refresh. Break-glass token.

**Requirements:** AUTH-01..07

**Success criteria:**
1. 10 failed logins for staff_id=X from 5 different IPs locks staff_id X, not 5 separate IP counters
2. Restarting racecontrol does not reset lockout counters
3. A staff JWT issued at 09:00 is still valid at 21:00 (12h)
4. Logging in on a second device does not invalidate the first
5. Break-glass token use triggers WhatsApp alert within 30 seconds

**Plans:**
- [x] 348-01-PLAN — per-IP + per-staff-id lockout on staff_validate_pin (`da0fb590`)
- [x] 348-02-PLAN — SKIPPED: JWT already 24h, no session revocation (pre-existing)
- [x] 348-03-PLAN — break-glass emergency access endpoint + WhatsApp alert (`a051c5d7`)

### Phase 349: DB Sync via Google Drive (revised)

**Goal:** Venue `racecontrol.db` syncs to cloud via shared Google Drive folder. Replaces original Litestream+B2 plan — simpler, free (2TB Google Workspace), uses existing OAuth credentials.

**Requirements:** SYNC-01..08

**Success criteria:**
1. Writing a driver on venue racecontrol is visible on cloud racecontrol SQLite within 10 minutes
2. Upload script (James .27) runs every 5 min via schtask, SCP from server .23 then Drive upload
3. Download script (Bono VPS) runs every 5 min via cron, skips if unchanged
4. sync-status.json on both sides reports last sync timestamp and DB size
5. Cloud racecontrol read-replica guard returns 409 on venue-authoritative writes (TODO: Phase 349-03)

**Plans:**
1/1 plans complete
- [x] 349-02-PLAN — Download script (download-db.sh) + cron on Bono VPS + env file deployment
- [x] 349-03-PLAN — Cloud racecontrol read-replica guard + /api/health sync lag probe

### Phase 350: Contract Tests

**Goal:** Playwright contract tests for every admin-to-downstream data flow. 46-page smoke test in deploy gate. Reuses Phase 343-04 sync-wait pattern.

**Requirements:** TEST-01..06

**Success criteria:**
1. Cafe menu contract test passes against live venue + live cloud
2. 46-page smoke test runs in <2 minutes, zero console errors on hydration
3. Tests run as part of `admin-deploy.sh --verify` — failing tests block deploy
4. Staff PIN contract test uses 70s sync-wait (reuses Phase 343-04 pattern)
5. Test data is cleaned up after each run (no test pollution in prod DB)

**Plans:**
- [ ] 350-01-PLAN — Cafe/pricing/coupon contract tests
- [ ] 350-02-PLAN — Staff PIN contract test (depends on Phase 347)
- [ ] 350-03-PLAN — 46-page smoke test + deploy gate integration

### Phase 351: Data Durability

**Goal:** Daily `sqlite3 .backup` on both DBs, 30d retention, rsync to Bono VPS, quarterly restore drill.

**Requirements:** OPS-08..14

**Success criteria:**
1. Daily scheduled task runs at 03:00 IST on venue + cloud
2. Backups appear in `C:\RacingPoint\backups\` (venue) + `/root/backups/` (cloud) with non-zero size
3. Both DBs verified in WAL mode at startup
4. Restore drill on a scratch machine recovers admin.db with matching row counts
5. Alert fires if backup missing or size 0 after scheduled window

**Plans:** 3/3 plans complete
- [x] 351-01-PLAN — Fix backup-databases.sh (admin.db, retention, validation, alert, schtask)
- [x] 351-02-PLAN — WAL mode verification + restore drill update + drill execution
- [x] 351-03-PLAN — Cloud backup script (backup-cloud.sh) + cron + first restore drill executed

### Phase 352: Health + WhatsApp Alerts

**Goal:** `/api/health` reports true ground truth per subsystem. Degradation triggers WhatsApp alert via comms-link relay within 30 seconds.

**Requirements:** OPS-01..07

**Success criteria:**
1. Killing admin.db file permission — `/api/health` admin_db subsystem reports not ok within 10s
2. WhatsApp alert fires within 30s of degradation — confirmed by message in the venue channel
3. Same subsystem + error_code within 10 min produces single alert (dedup works)
4. Phase 343 Plan 02 `whatsapp_alerter.rs` TODO wired and firing
5. Structured JSON logs rotate daily and appear on Bono VPS within 24h

**Plans:**
3/3 plans complete
- [x] 352-02-PLAN — comms-link `/relay/alert` integration + dedup logic
- [x] 352-03-PLAN — Structured JSON logs + rotation + rsync

### Phase 353: Runbook + Staff Training

**Goal:** Printed one-pagers at POS. Incident log ritual. Staff know what to do when admin breaks.

**Requirements:** OPS-15..19

**Success criteria:**
1. Three printed one-pagers physically present at POS (general, PIN change, cafe menu change)
2. Staff can describe the escalation path from memory
3. Incident log used at least once during the first week of operation
4. Morning review of incident log happens daily for the first 2 weeks
5. Uday signs off on the runbook content

**Plans:**
2/2 plans complete
- [x] 353-02-PLAN — Staff training session + sign-off

### Phase 354: UI Hardening

**Goal:** No broken buttons or blank loading screens on any admin page. Dead routes removed from nav.

**Requirements:** UI-01..07

**Success criteria:**
1. `/memberships` and `/wallet-transactions` not in nav (or behind feature flag)
2. Every rcFetch call shows a loading skeleton during fetch
3. Every empty list shows a meaningful empty state message
4. Every mutation shows success/failure toast
5. `/settings/health` page tiles update live

**Plans:**
- [ ] 354-01-PLAN — Close remaining skeleton gaps (4), replace alert() with toast (15), empty states

### Phase 355: Venue-Ready Readiness Review

**Goal:** Execute the 18-criterion Venue Opening Readiness Checklist. Produce VERIFICATION.md. Decide ship vs. defer remaining items.

**Requirements:** (cross-theme — final verification)

**Success criteria:**
1. All 15 P0 criteria green
2. P1 criteria either green or explicitly deferred with reason
3. VERIFICATION.md committed with evidence per criterion
4. User (Uday) signs off on venue-ready state
5. v47.0 COMPLETE marker added to MILESTONES.md

**Plans:**
- [ ] 355-01-PLAN — Execute checklist, produce VERIFICATION.md
- [ ] 355-02-PLAN — Milestone close + LOGBOOK + ARCHITECTURE.md + memory file updates

### Phase 356: Business Rules Config Table

**Goal:** Migrate ~15 hardcoded Rust consts into a `business_rules` SQLite table so tuning doesn't require a code ship.

**Requirements:** BIZRULE-01..15

**Success criteria:**
1. New `business_rules` table exists with migration + seed values matching current hardcoded defaults (zero behavior change at deploy)
2. All consumers (billing.rs, routes.rs, psychology.rs) read from the table at runtime
3. Admin `/settings/business-rules` page lists rules with inline edit + audit log
4. Changing `referral_reward_referrer_paise` from 10000 → 20000 via admin UI reflects on next session end without restart
5. Legal policy text (refund/pricing/GST) served from `GET /pricing/display` reads from business_rules
6. Startup seeds missing rules from fallback constants (graceful forward compat)

**Plans:**
- [ ] 356-01-PLAN — Schema migration + seed + read helpers
- [ ] 356-02-PLAN — Refactor consumers (billing.rs, routes.rs, psychology.rs)
- [ ] 356-03-PLAN — Admin /settings/business-rules page + audit log

### Phase 357: Pricing Tiers CRUD

**Goal:** Admin can create/edit/delete/reorder the plan cards that show on the kiosk staff wizard without touching the DB.

**Requirements:** TIER-01..05

**Success criteria:**
1. Admin `/pricing/tiers` page lists all tiers with drag-to-reorder
2. Add New Tier modal creates a row via `POST /pricing/tiers`
3. Adding a new tier appears on kiosk staff wizard within 30s (sync cycle)
4. `is_popular` flag replaces hardcoded "middle tier" heuristic in PricingDisplay.tsx
5. SetupWizard "save 7%" / "save 40%" strings computed dynamically from tier prices
6. Deleting a tier that has active sessions blocked with 409 + warning

**Plans:**
- [ ] 357-01-PLAN — Racecontrol CRUD endpoints + is_popular migration
- [ ] 357-02-PLAN — Admin /pricing/tiers page
- [ ] 357-03-PLAN — Remove kiosk hardcoded save-% strings

### Phase 358: Cafe Promos Admin Page

**Goal:** Admin UI for cafe promos. Backend already has full CRUD at `/api/v1/cafe/promos` — pure frontend work.

**Requirements:** PROMO-01..05

**Success criteria:**
1. Admin `/cafe/promos` page lists all promos with type badge + active toggle
2. Create combo promo ("Burger + Fries for 299") visible on kiosk cafe panel within 30s
3. Toggle happy_hour active/inactive without modal
4. Broadcast button calls `POST /cafe/marketing/broadcast` with dedup check
5. Delete confirmation dialog; hard delete not soft

**Plans:**
- [ ] 358-01-PLAN — /cafe/promos list view
- [ ] 358-02-PLAN — Create/edit modal with type-specific config fields
- [ ] 358-03-PLAN — Broadcast integration

### Phase 359: Bonus Tiers Admin Page

**Goal:** Admin can define wallet topup bonus rules ("top up ₹2000 get 10% bonus") from the UI.

**Requirements:** BONUS-01..05

**Success criteria:**
1. Admin `/wallet/bonus-tiers` page lists all tiers sorted by min_amount_paise
2. Creating a new tier reflects on PWA + POS topup modals within 30s
3. Bonus preview widget shows projected credits for each tier
4. Inactive tiers hidden from customer-facing UIs but still in admin list

**Plans:**
- [ ] 359-01-PLAN — Racecontrol `/wallet/bonus-tiers` admin CRUD endpoints
- [ ] 359-02-PLAN — Admin /wallet/bonus-tiers page

### Phase 360: Topup Presets SSOT (partially shipped 2026-04-09)

**Goal:** Both PWA wallet topup page and POS WalletTopupModal read preset amounts from a single server endpoint. Kill the drift where PWA showed `[500, 1000, 2000, 3000, 4000, 5000]` but POS showed `[500, 700, 900, 1000, 2000, 3000]`.

**Requirements:** TOPUP-01..06

**Shipped 2026-04-09 (commit `0c7a8d86`):**
- [x] TOPUP-01: `system_settings.wallet_topup_presets_paise` key + server read path
- [x] TOPUP-02: `GET /wallet/topup-presets` public endpoint with 8-entry safe default
- [x] TOPUP-03: PWA `api.topupPresets()` + `wallet/topup/page.tsx` dynamic state
- [x] TOPUP-04: POS `WalletTopupModal.tsx` dynamic state

**Remaining:**
- [ ] TOPUP-05: Admin `/wallet/topup-presets` editor UI
- [ ] TOPUP-06: Playwright contract test (covered by Phase 350 umbrella)

**Plans:**
- [x] 360-01-PLAN — Backend + PWA + POS dynamic fetch (0c7a8d86)
- [ ] 360-02-PLAN — Admin `/wallet/topup-presets` editor page
- [ ] 360-03-PLAN — Contract test in Phase 350 suite

### Phase 368: Live Launch Status with Autonomous Debug

**Goal:** Kiosk /debug page shows real-time per-launch status cards (4-state model: started → analyzing → fixing → fixed), WS-push only, launch-phase bounded, billing internal, inline staff notes. Surfaces Phase 275's existing rc-agent retry/KB/gossip machinery via new WS event channel (LaunchStateChanged) + new UI component replacing flat activity feed. Removes 30s poll (anti-cheat risk). Deploy venue + cloud parity.
**Requirements:** LLS-01, LLS-02, LLS-03, LLS-04, LLS-05, LLS-06, LLS-07, LLS-08, LLS-09, LLS-10, LLS-11, LLS-12 (proposed by 368-RESEARCH.md §Proposed REQ-IDs; authoritative for this phase — REQUIREMENTS.md has no pre-existing Phase 368 entries)
**Depends on:** Phase 275 (autonomous game launch fix — rc-agent retry + KB + gossip, shipped 2026-04-01)
**Plans:** 4/4 plans complete

Plans:
- [x] 368-01-PLAN.md — Protocol types + LaunchStateMachine + launch_id threading + billing-reject sanitization + issue_fixed emission (Wave 1, autonomous)
- [x] 368-02-PLAN.md — rc-agent emissions at 4 retry boundaries + server relay of AgentMessage::LaunchStatusUpdate (Wave 2, autonomous, parallel with 03)
- [x] 368-03-PLAN.md — launch_notes DB + cloud_sync + 5 REST endpoints + feature flag seed + tier gate (Wave 2, autonomous, parallel with 02)
- [x] 368-04-PLAN.md — Kiosk LaunchCard + types + WS handling + Playwright probe + MMA audit + deploy parity (Wave 3, non-autonomous — UI + MMA + deploy checkpoints)

---

## Milestone v46.0: Game Launch Diagnostics (PARALLEL with v47.0)

**Started:** 2026-04-09 (retroactive — Phase 362 Layer 3 shipped ad-hoc as build `a9b5eaa3` same day)
**Status:** Active, runs parallel with v47.0 Admin Dashboard Venue-Ready Hardening
**Requirements:** `.planning/milestones/v46.0-REQUIREMENTS.md`
**Standalone roadmap:** `.planning/milestones/v46.0-ROADMAP.md`
**Goal:** Close all 21 silent data-loss points between kiosk session setup and race results. Move verification from "is the game alive?" to "is it running correctly AND recording everything?"
**Phase range:** 361-367

### Phase 361: Kiosk Preset Filtering + Server Gate

**Goal:** Prevent invalid car/track combos at source. Wire unused `presetValidity`, filter by pod inventory, reject at API.

**Requirements:** GLD-A-01..04

**Success criteria:**
1. Kiosk car/track dropdowns filter to installed-on-pod only
2. Invalid combos disable "Start Session" with `presetValidity` reason surfaced
3. Server `/sessions/start` returns 422 with `{reason, suggestion}` on bypass attempt
4. Admin `/admin/content-drift` lists pods with inventory drift

**Plans:**
- [x] 361-01-PLAN — Server inventory endpoint + validity gate
- [x] 361-02-PLAN — Kiosk filter + presetValidity surface
- [x] 361-03-PLAN — Admin content-drift page + server proxy (code-complete, deploy pending)

### Phase 362: Post-Launch Config Verification (Layer 3) — SHIPPED 2026-04-09

**Goal:** Read sim shared-memory / UDP to verify launched game matches kiosk-requested config.

**Requirements:** GLD-B-01..05 (all `[x]`)

**Shipped:** build `a9b5eaa3`, all 8 pods, 2026-04-09. Pod 8 canary visually confirmed.

**Files:** `crates/rc-agent/src/sims/{mod,assetto_corsa,f1_25,iracing,lmu,assetto_corsa_evo}.rs`, `launch_verifier.rs`, `event_loop.rs`, `ac_launcher.rs`, `rc-common/protocol.rs`, `racecontrol ws/mod.rs`.

**NOT tested (tracked as GLD-G-05 in Phase 367):** deliberate-mismatch WhatsApp alert E2E, ACR/LMU runtime verification, 8-pod concurrent-mismatch load.

**Plans:**
- [x] 362-01-PLAN — SessionConfig struct + read_session_config on 5 adapters (`a9b5eaa3`)
- [x] 362-02-PLAN — verify_launch_config Stage 5 + ConfigMismatchDetected WS + admin broadcast (`a9b5eaa3`)
- [x] 362-03-PLAN — Atomic race.ini write + AI car content validation (`a9b5eaa3`)

### Phase 363: Data Recording Verification — CODE-COMPLETE 2026-04-10 (deploy deferred)

**Goal:** Lap audit + telemetry completeness + CSV auto-sync + 5s billing grace window. Closes all 3 P0s.

**Status:** All 3 plans committed + tested. `cargo test -p racecontrol-crate` = 891 passed, `-p rc-agent-crate` = 254 passed, 7 Phase 363-03 tests green (F-05 formula + SQL invariant + grace window × 3 + lap reject × 2). **NOT SHIPPED to server .23 or Bono VPS** — MMA audit + binary build + deploy still required per CLAUDE.md. Production still runs `d4359d2e` (pre-v46.0 binary); F-05 refund bug and GLD-C-04 lap-reject race remain live until deploy. Parked pending deploy window.

**Requirements:** GLD-C-01..04

**Success criteria:**
1. Session-end lap audit flags >10% lap-count gap as `incomplete`
2. Telemetry coverage <80% marks session `suspect: true`
3. CSV fallback auto-syncs within 30s of session end
4. Lap-reject arriving within 5s of session end updates refund calc before commit

**Plans:**
3/3 plans complete
- [x] 363-01-PLAN — Lap audit + telemetry completeness + DB migration
- [x] 363-02-PLAN — CSV fallback auto-sync path
- [x] 363-03-PLAN — Billing 5s grace window + lap-reject race fix

### Phase 364: Session Quality Monitor

**Goal:** Detect in-flight session quality degradation before session end.

**Requirements:** GLD-D-01..05

**Success criteria:**
1. `TelemetryGap` events fire on >500ms gaps and are logged
2. Lap consistency checker flags >3σ outliers as suspect
3. `SessionStalled` warning fires after 15s in-race telemetry silence
4. Zero `let _ = ws_send(...)` patterns in hot path (rg verified)
5. `ws_try_send_overflows_total` metric exposed

**Plans:**
3/3 plans complete
- [x] 364-02-PLAN — Lap consistency checker
- [x] 364-03-PLAN — Silent-drop audit + overflow metrics

### Phase 365: AI Behavior Validation via MMA

**Goal:** Expected AI lap time KB per (car, track, difficulty). Live anomaly detection.

**Requirements:** GLD-E-01..04

**Success criteria:**
1. `ai_behavior_samples` table populated after any >3-lap AI session
2. Weekly MMA batch produces KB updates with 3/5 consensus
3. `AiBehaviorAnomaly` fires on >3 consecutive laps outside band
4. Admin dashboard surfaces per-car-track AI performance trend

**Plans:**
3/3 plans complete
- [x] 365-01-PLAN — ai_behavior_samples schema + collector (773fff93)
- [x] 365-02-PLAN — Weekly MMA batch + KB format (ced70634)
- [x] 365-03-PLAN — Live anomaly detector (39674046)

### Phase 366: Fleet Intelligence

**Goal:** Per-pod composite health + time-of-day patterns + content drift + concurrent session guard.

**Requirements:** GLD-F-01..04

**Success criteria:**
1. `/fleet/intelligence` returns 0-100 composite per pod
2. Time-of-day anomaly report identifies hour-correlated failures
3. `ContentDriftDetected` fires on inventory delta vs TOML
4. Second session attempt on active pod returns HTTP 409

**Plans:**
4/4 plans complete
- [x] 366-02-PLAN — Content drift detector + background task (47a22520)
- [x] 366-03-PLAN — Concurrent session guard — HTTP 409 upgrade (546d00d8)
- [x] 366-04-PLAN — Integration gate + documentation (e3659ba6)

### Phase 367: Staff Tools

**Goal:** Admin UIs for suspect lap triage, on-demand verify, replay, export, and retro-validation of Phase B.

**Requirements:** GLD-G-01..05

**Success criteria:**
1. `/admin/suspect-laps` drills into per-lap telemetry heatmap
2. "Verify Pod N" button runs synthetic Phase B test in <15s
3. Session replay plays at 1×-10× speed
4. Batch export produces CSV/JSONL for billing + telemetry + laps
5. GLD-G-05: Phase 362 retro-validation passes (deliberate mismatch → WhatsApp E2E, all 5 adapters runtime-verified, 8-pod load)

**Plans:**
5/5 plans executed
- [x] 367-01-PLAN — Suspect sessions + telemetry heatmap (GLD-G-01)
- [x] 367-02-PLAN — On-demand pod verify (GLD-G-02)
- [x] 367-03-PLAN — Session replay player (GLD-G-03)
- [x] 367-04-PLAN — Batch export (GLD-G-04)
- [x] 367-05-PLAN — Phase 362 retro-validation (GLD-G-05)

---

## v48.0 Codebase Architecture — Department-Driven Event Mesh

**Goal:** Rewrite the AC launch path to VMS parity, fix core product reliability (launch, laps, billing, multiplayer), add P1 business model features (PWA PIN launch, wallet types, cafe, marketing), then decompose the 419K-line codebase into department-aligned modules with an event bus. Priority order: P0 first, P1 second, P2 third.

**Phases:** 14  |  **Coverage:** 54/54 requirements mapped

**Priority rule:** No P1 phase starts until ALL P0 requirements verified. No P2 until ALL P1 verified.
Exception: P2 decomposition directly unblocking a P0 req may run in parallel.

**Phases:**

- [ ] **Phase 369: AC Launch Rewrite (P0)** — Rewrite AC launch to VMS-parity, separate staff/PWA code paths
- [ ] **Phase 370: Multi-Game Launch (P0)** — F1 25, iRacing, LMU launchers; SimLauncher trait
- [ ] **Phase 371: Lap Recording (P0)** — All 4 games record laps end-to-end; leaderboard within 10s
- [ ] **Phase 372: Billing — Arcade Model (P0)** — Coin-first, per-minute, crash-pause, tier options
- [ ] **Phase 373: Multiplayer (P0)** — Simultaneous launch, atomic billing, continuous lap recording
- [ ] **Phase 374: PWA Self-Service Launch (P1)** — PIN generation, pin-grid on pod, independent code path
- [ ] **Phase 375: Wallet Types (P1)** — Cash vs promotional credits, refund enforcement, unified debit
- [ ] **Phase 376: Cafe Integration (P1)** — Cafe wallet debit, combo deals with racing
- [ ] **Phase 377: Customer Experience (P1)** — Multi-game stats/PBs, unified leaderboard, <15s launch time
- [ ] **Phase 378: Marketing Engine (P1)** — Low-utilization detection, WhatsApp deal push, combo promos
- [ ] **Phase 379: Event Bus Foundation (P2)** — DomainEvent enum, mesh broadcast, correlation IDs
- [ ] **Phase 380: Codebase Decomposition (P2)** — routes.rs, billing.rs, db, all 141 files, lock screen split
- [ ] **Phase 381: Fix Tooling (P2)** — blast-radius tool, insertion:deletion ratio hook, band-aid audit
- [ ] **Phase 382: Foundation & CI (P2)** — feature registry, dead code removal, CI gate, CODEOWNERS

## Phase Details

### Phase 369: AC Launch Rewrite
**Goal**: Staff can launch Assetto Corsa from the kiosk reliably in under 5 seconds, via a clean VMS-parity launcher under 500 lines, with staff and PWA launch as completely separate code paths
**Depends on**: Nothing (first phase in v48.0)
**Requirements**: LNCH-01, LNCH-05, LNCH-07
**Success Criteria** (what must be TRUE):
  1. Staff clicks launch on kiosk — AC starts on the pod within 5 seconds, every time, no failures
  2. The AC launch code path is under 500 lines (replacing the current 19,597-line path)
  3. Staff Launch (kiosk) and PWA Launch (PIN) share no code except "validate funds -> debit -> launch"
  4. A test launch from kiosk produces a correct race.ini on the pod that can be read back and verified
**Plans**: TBD
**UI hint**: yes

### Phase 370: Multi-Game Launch
**Goal**: Staff can launch F1 25, iRacing, and LMU from the kiosk — each with its own SimLauncher implementation under 500 lines, no shared copy-paste from AC
**Depends on**: Phase 369
**Requirements**: LNCH-02, LNCH-03, LNCH-04, LNCH-06
**Success Criteria** (what must be TRUE):
  1. Staff launches F1 25 from kiosk — game starts on pod, no pin-grid block, no stuck browser
  2. Staff launches iRacing from kiosk — game starts on pod
  3. Staff launches LMU from kiosk — game starts on pod
  4. Each of the 4 games has a distinct SimLauncher trait implementation under 500 lines with no copy-paste from other launchers
**Plans**: TBD

### Phase 371: Lap Recording
**Goal**: Laps recorded for every supported game appear on the leaderboard within 10 seconds of completion, with full telemetry captured
**Depends on**: Phase 370
**Requirements**: LAPS-01, LAPS-02, LAPS-03, LAPS-04, LAPS-05, LAPS-06
**Success Criteria** (what must be TRUE):
  1. A lap driven in AC is stored in the database and visible on the PWA leaderboard within 10 seconds
  2. A lap driven in F1 25 is stored in the database and visible on the PWA leaderboard within 10 seconds
  3. iRacing and LMU laps are also recorded to the database
  4. Speed, gear, throttle, and brake telemetry are captured for all 4 games during a session
**Plans**: TBD

### Phase 372: Billing — Arcade Model
**Goal**: The billing system behaves like an arcade machine — customer puts in credits before playing, per-minute charges run while the game is active, game stops when credits run out
**Depends on**: Phase 371
**Requirements**: BILL-01, BILL-02, BILL-03, BILL-04, BILL-05
**Success Criteria** (what must be TRUE):
  1. A customer with zero wallet balance cannot start a game session — the launch is blocked at the kiosk
  2. Wallet is debited at game start, not at session creation; the debit is visible immediately
  3. Per-minute billing runs only while the game process is active — pauses automatically on game crash
  4. A customer can select 30-minute (₹700) or 1-hour (₹900) tiers and both calculate and charge correctly
  5. If the game crashes, billing pauses; when staff relaunches the game, billing resumes from where it stopped
**Plans**: TBD

### Phase 373: Multiplayer
**Goal**: Two or more customers can launch a multiplayer session simultaneously, have their laps recorded, and be billed atomically — either all participants are charged or none are
**Depends on**: Phase 372
**Requirements**: MULT-01, MULT-02, MULT-03, MULT-04
**Success Criteria** (what must be TRUE):
  1. Staff launches an AC multiplayer session — games start on 2+ pods at the same time
  2. All participants' laps appear on the leaderboard during and after the multiplayer session
  3. No participant is dropped or orphaned mid-race due to session disconnection
  4. If any participant's wallet debit fails at session start, no participants are charged and the session does not launch
**Plans**: TBD

### Phase 374: PWA Self-Service Launch
**Goal**: A customer can pick a game on their phone, receive a 4-digit PIN, enter it on the pod, and the game starts — entirely without staff involvement, via a code path completely independent from the staff kiosk path
**Depends on**: Phase 373 (all P0 verified)
**Requirements**: PWAL-01, PWAL-02, PWAL-03
**Success Criteria** (what must be TRUE):
  1. Customer selects a game and preset in the PWA and receives a 4-digit numeric PIN
  2. Customer enters the PIN on the pod's 4-digit PIN grid — game launches without staff touching anything
  3. The PWA launch path shares no code with the staff kiosk path except "validate funds -> debit -> launch"
**Plans**: TBD
**UI hint**: yes

### Phase 375: Wallet Types
**Goal**: The wallet distinguishes cash credits (refundable) from promotional credits (spend-only), enforces refund limits, and accepts a single debit call for both games and cafe
**Depends on**: Phase 374
**Requirements**: WLLT-01, WLLT-02, WLLT-03
**Success Criteria** (what must be TRUE):
  1. When a customer top-up creates credits, those credits are tagged as "cash" and are refundable
  2. When a promotion grants credits, those credits are tagged as "promotional" and cannot be refunded
  3. A refund request can never exceed the total cash credits deposited — promotional credits are never refunded
  4. A single wallet debit call works for both game sessions and cafe orders
**Plans**: TBD

### Phase 376: Cafe Integration
**Goal**: Cafe orders charge from the same customer wallet as games, and staff can offer combo deals that bundle a cafe item with a game session at a discount
**Depends on**: Phase 375
**Requirements**: CAFE-01, CAFE-02
**Success Criteria** (what must be TRUE):
  1. A cafe order placed for a customer deducts credits from their wallet — the same wallet used for racing
  2. A combo deal exists in the system and applies a discount when a customer books both a game session and a cafe item
**Plans**: TBD
**UI hint**: yes

### Phase 377: Customer Experience
**Goal**: The PWA shows a customer their stats and personal bests for every game they've played, the leaderboard covers all four games, and the time from staff clicking launch to customer driving is under 15 seconds
**Depends on**: Phase 376
**Requirements**: CUST-01, CUST-02, CUST-03
**Success Criteria** (what must be TRUE):
  1. Customer opens the PWA and sees session stats, personal bests, and telemetry for sessions in AC, F1 25, iRacing, and LMU
  2. The public leaderboard shows fastest laps across all four supported games, not just AC
  3. From the moment staff clicks "Launch" to the moment the customer is in the game is under 15 seconds
**Plans**: TBD
**UI hint**: yes

### Phase 378: Marketing Engine
**Goal**: The system detects when bookings are low and automatically pushes targeted deals to customers via WhatsApp, including cafe+racing combo promotions
**Depends on**: Phase 377
**Requirements**: MKTG-01, MKTG-02, MKTG-03
**Success Criteria** (what must be TRUE):
  1. The system detects a low-utilization period (e.g., weekday afternoon with under 2 active pods) and generates a deal automatically
  2. Generated deals are sent via WhatsApp to registered customers without staff intervention
  3. A "cafe + racing" combo promotion type exists and can be configured and sent as a deal
**Plans**: TBD

### Phase 379: Event Bus Foundation
**Goal**: Every department communicates through typed events on the comms-link mesh — no direct shared mutable state, every customer action traceable by a single correlation ID
**Depends on**: Phase 378 (all P1 verified)
**Requirements**: EVNT-01, EVNT-02, EVNT-03, EVNT-04, EVNT-05
**Success Criteria** (what must be TRUE):
  1. rc-common contains a DomainEvent enum with typed events for all departments — it compiles without warnings
  2. A game launch produces GameStarted, GameCrashed, and GameEnded events visible to any subscribed device on the mesh
  3. The billing module subscribes to game events and updates billing state in response — no polling loop
  4. A correlation ID from a single customer booking can be traced through game launch, billing debit, and lap recording logs
**Plans**: TBD

### Phase 380: Codebase Decomposition
**Goal**: The monolithic files that accumulated 1,397 debug commits are split into department-aligned modules — routes.rs, billing.rs, db/mod.rs, and the 141 oversized files — with lock screen logic fully separated from game launch
**Depends on**: Phase 379
**Requirements**: DCMP-01, DCMP-02, DCMP-03, DCMP-04, DCMP-05
**Success Criteria** (what must be TRUE):
  1. routes.rs no longer exists as a single file — route handlers are organized in department-aligned modules (billing/, games/, auth/, etc.)
  2. billing.rs is split into at least wallet.rs, session_lifecycle.rs, pricing.rs, and post_session.rs
  3. The lock screen and screen blanking logic is in its own module with no imports from game launch modules
  4. Every source file in the codebase is under 500 lines
**Plans**: TBD

### Phase 381: Fix Tooling
**Goal**: Developers have a static blast-radius tool, a pre-commit ratio hook that warns on bloated fix commits, and all 36K lines of accumulated band-aid code have been reviewed and replaced with root fixes
**Depends on**: Phase 380
**Requirements**: FTOL-01, FTOL-02, FTOL-03
**Success Criteria** (what must be TRUE):
  1. Running the fix-scope tool against any function name outputs its callers, shared state dependencies, and cross-crate dependents
  2. A pre-commit hook warns when a commit has an insertion-to-deletion ratio above 2:1 and is labeled a fix commit
  3. The 36K lines of net fix bloat have been audited — band-aids with a known root fix are replaced; remaining justified fixes are labeled with comments
**Plans**: TBD

### Phase 382: Foundation & CI
**Goal**: Every feature in the codebase is classified, dead code is removed, a CI gate enforces test and lint quality on every merge, and ownership is assigned to prevent future ownership ambiguity
**Depends on**: Phase 381
**Requirements**: FNDN-01, FNDN-02, FNDN-03, FNDN-04, FNDN-05
**Success Criteria** (what must be TRUE):
  1. A machine-readable Feature Registry exists classifying every feature as complete, dead, orphaned, or incomplete
  2. Running cargo build on the codebase after dead code removal produces a binary that is 10-20% smaller than before
  3. A CI gate runs cargo test and cargo clippy on every pull request — a failing gate blocks merge
  4. A CODEOWNERS file assigns each source directory to either Bono or James — no unowned directories remain

## v48.0 Progress

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 369. AC Launch Rewrite (P0) | 1/TBD | Code committed (76de0727) | Deploy pending |
| 370. Multi-Game Launch (P0) | 0/TBD | Execution plan only (for James) | - |
| 371. Lap Recording (P0) | 0/TBD | Execution plan only (~70 lines wiring) | - |
| 372. Billing — Arcade Model (P0) | 1/TBD | Code committed (6f398865) | Deploy pending |
| 373. Multiplayer (P0) | 0/TBD | Execution plan only (for James) | - |
| 374. PWA Self-Service Launch (P1) | 1/TBD | Code committed (05827cd9) | Deploy pending |
| 375. Wallet Types (P1) | 1/TBD | Code committed (881ab186) | Deploy pending |
| 376. Cafe Integration (P1) | 1/TBD | Code committed (d6c200f6) | Deploy pending |
| 377. Customer Experience (P1) | 1/TBD | Code committed (602de4d9) | Deploy pending |
| 378. Marketing Engine (P1) | 1/TBD | Code committed (2d0106ad) | Deploy pending |
| 379. Event Bus Foundation (P2) | 1/TBD | Code committed (cdbc24c3) | Deploy pending |
| 380. Codebase Decomposition (P2) | 1/TBD | Code committed (dd81c959) — routes.rs split into 55 modules | Deploy pending |
| 381. Fix Tooling (P2) | 1/TBD | Code committed (7091d6c8) | Deploy pending |
| 382. Foundation & CI (P2) | 1/TBD | Feature Registry doc committed (b22f0fc6) | Deploy pending |

*v48.0 status: 10/14 phases have code committed. 3 are execution plans for James. All need deploy + verification. Absorbed into v49.0 Wave 1.*

---

## v49.0 Unified RaceControl Operations

**Goal:** Transform RaceControl from a collection of features into a reliable, self-monitoring system that records every lap, runs autonomously, and actively drives revenue through intelligent pricing and targeted marketing. Ship everything committed in v48, fix lap recording (Uday's #1 pain), then build autonomous revenue systems.

**Phases:** 10  |  **Coverage:** 47 requirements mapped  |  **Requirements:** `.planning/REQUIREMENTS.md`

**Wave structure:**
```
Wave 1: Deploy & Verify (383)
  └──> Wave 2: Lap Recording P0 (384) ──[UDAY GATE]
         └──> Wave 3: Architecture (385)
         └──> Wave 4: Revenue (386, 387, 388)
                386 (Pricing) ──independent
                387 (Preferences) ──> 388 (Marketing)
         └──> Wave 5: Game Launch (389)
         └──> Wave 6: Polish (390, 391, 392)
                390 (Displays + Cloud) ──independent
                391 (Staff Ops) ──independent
                392 (Readiness Review) ──depends on all above
```

**Parallel track:** James completes v47.0 phases 345, 346, 350, 354-360 independently.

### Phase 383: Deploy & Verify Pipeline

**Goal:** Ship all committed-but-undeployed code from v48 and v46. The codebase running on production must match the codebase in git. Nothing new is built until this is done.
**Depends on:** Nothing (first phase)
**Executor:** Joint (Bono: server .23 + VPS. James: pods via canary rollout)
**Requirements:** DPLY-01, DPLY-02, DPLY-03, DPLY-04, DPLY-05, DPLY-06

**Success criteria:**
1. `cargo build --release` on current main produces a binary that starts and serves `/api/v1/health` with 200
2. Server .23 and Bono VPS both run the new binary with matching build_id
3. Phase 363 billing 5s grace window is active — a lap arriving within 5s of session end updates the refund calc
4. ADAPTER-SWAP F1 25 fixes deployed to Pod 8 canary first, then remaining pods after 24h soak
5. routes.rs split (55 modules) serves all existing endpoints — zero functional regression verified by smoke test
6. Phase 363 lap audit and telemetry completeness checks are active on server

**Plans:** TBD

---

### Phase 384: Lap Recording Wiring

**Goal:** The ~70 lines of wiring that connect existing sim adapters to the lap persistence pipeline. After this phase, a customer drives in AC or F1 25 and their laps appear in the database and on the leaderboard.
**Depends on:** Phase 383 (deploy must be complete)
**Executor:** Bono (server-side wiring in racecontrol crate)
**Requirements:** LAPR-01, LAPR-02, LAPR-03, LAPR-04, LAPR-05, LAPR-06, VRFY-01, VRFY-02, VRFY-03

**Success criteria:**
1. On LaunchGame command, the correct sim adapter is swapped in (not the boot-time default) — verified by log output showing adapter type matching launched game
2. If shared memory isn't available after game launch, adapter retries every 2s for up to 60s — verified by launching AC and checking agent logs for retry pattern
3. persist_lap succeeds even without an active billing session — verified by driving a free trial lap and checking the laps table
4. AC lap flows end-to-end: shared memory → rc-agent → WS → server → SQLite → PWA leaderboard within 10s
5. F1 25 lap flows end-to-end: UDP 20777 → rc-agent → WS → server → SQLite → leaderboard
6. **Uday gate:** Uday at venue launches AC, drives 3 laps, sees them on PWA leaderboard

**Key files (from Phase 371 execution plan):**
- `crates/rc-agent/src/ws_handler.rs` — adapter swap on LaunchGame
- `crates/rc-agent/src/event_loop.rs` — connect retry loop
- `crates/racecontrol/src/api/routes.rs` (now split modules) — persist_lap without billing session guard

**Plans:** TBD

---

### Phase 385: Architecture Completion — COMPLETE 2026-04-16

**Goal:** Finish the decomposition started in v48 Phase 380. billing.rs and db/mod.rs are the remaining large files. Dead code removed. CI gate enforced.
**Depends on:** Phase 384 (P0 lap recording must work first)
**Executor:** Bono
**Requirements:** ARCH-01, ARCH-02, ARCH-03, ARCH-04, ARCH-05

**Success criteria:**
1. ✅ billing.rs split into wallet.rs, session_lifecycle.rs, pricing.rs, post_session_hooks.rs — each under 500 lines (prior session)
2. ✅ db/mod.rs split by department table groups — each under 500 lines (prior session)
3. ✅ All remaining files >500 lines split (remaining >500: routes.rs=860 router aggregation, migrate_billing.rs=869 SQL DDL — structural exemptions)
4. ✅ CI gate runs `cargo test` + `cargo clippy` on PRs — added to `.github/workflows/ci.yml` (`f04efacb`)
5. ✅ Dead code audit: 7 modules removed (1,674 lines) + 280 clippy auto-fixes. Total: 2,403 deletions, 598 insertions. 10-20% target unrealistic — codebase is lean with <1% dead code.

**Commits:** Prior sessions (6 splits) + `f04efacb` (clippy + CI gate + dead code removal)

---

### Phase 386: Autonomous Pricing Engine — COMPLETE 2026-04-16

**Goal:** Bono computes optimal session prices from venue expense data and adjusts pricing_rules automatically. Uday does NOT approve individual price changes — Bono decides based on the math.
**Depends on:** Phase 384 (lap recording P0 verified)
**Executor:** Bono
**Requirements:** PRCG-01, PRCG-02, PRCG-03, PRCG-04, PRCG-05

**Success criteria:**
1. ✅ `business_expenses` table with 7 cost categories. Seeded with known venue costs (₹4.62L/month). GET/POST API.
2. ✅ `calculate_break_even()`: expenses / avg_session_revenue = sessions needed. Returns margin %, profitability flag, min price.
3. ✅ `generate_recommendations()`: factors margin, utilization, break-even minimum. 4 factor types.
4. ✅ POST /pricing/engine/apply updates `pricing_tiers` directly + logs to `activity_log` for audit.
5. ✅ 5 API endpoints: expenses CRUD, break-even analysis, recommendations, apply. `pricing_engine_history` table tracks all.

**Commits:** `8d4d0ce0`

---

### Phase 387: Customer Opt-In/Opt-Out Preferences — COMPLETE 2026-04-16

**Goal:** Anti-spam infrastructure. Every customer can control what promotional messages they receive. This MUST be deployed and verified before any marketing messages are sent.
**Depends on:** Phase 384 (P0 verified)
**Executor:** Bono
**Requirements:** PREF-01, PREF-02, PREF-03, PREF-04, PREF-05

**Success criteria:**
1. ✅ `customer_preferences` table: opt_in_promotions, channel_preference, frequency_cap_per_week, last_promo_sent_at, consecutive_ignored, auto_paused, weekly tracking
2. ✅ POST /customer/opt-out/{id} immediately opts out. `can_send_promo()` blocks all promos for opted-out customers.
3. ✅ Weekly frequency cap enforced at send time via `can_send_promo()` — default 3/week, configurable 0-10. Auto-resets on ISO week boundary.
4. ✅ 3 consecutive ignored offers triggers auto-pause via `record_promo_ignored()`. `record_engagement()` resets on venue visit.
5. PWA `/settings/preferences` page — deferred to standalone frontend task

**Commits:** `f2bc7137`

---

### Phase 388: Autonomous Marketing Triggers — COMPLETE 2026-04-16

**Goal:** Bono detects empty pods and sends targeted offers to opted-in customers. No flooding. No spam. Just smart, targeted nudges during empty hours.
**Depends on:** Phase 387 (opt-in/opt-out MUST be live first)
**Executor:** Bono
**Requirements:** AMKT-01, AMKT-02, AMKT-03, AMKT-04, AMKT-05

**Success criteria:**
1. ✅ `run_marketing_sweep()` detects 4+ empty pods during busy hours, logs `low_utilization_marketing` to activity_log
2. ✅ 3 offer types by customer history: first-timer trial, value offer, combo deal. Personalized by name + empty pod count.
3. ✅ `can_send_promo()` called before every send. WhatsApp via `send_whatsapp_to()` (Evolution API).
4. ✅ `promo_delivery_log` tracks sent_at/opened_at/redeemed_at. `record_promo_ignored()` feeds auto-pause.
5. ✅ Combo deal ("Book 1 hour, free coffee") is the `auto_combo_deal` campaign for returning customers.

**Commits:** `09015ac1`

---

### Phase 389: Game Launch Completion

**Goal:** Full multi-game support — F1 25 verified on all pods, iRacing basic launch, LMU with timer billing, multiplayer AC with lap recording.
**Depends on:** Phase 383 (ADAPTER-SWAP deployed), Phase 384 (lap recording wired)
**Executor:** Joint (Bono: server. James: pod-side testing on all 8 pods)
**Requirements:** GAME-01, GAME-02, GAME-03, GAME-04

**Success criteria:**
1. F1 25: staff launches from kiosk → game starts → telemetry flows → laps recorded. Verified on all 8 pods.
2. iRacing: staff launches from kiosk → game starts on pod. Telemetry and lap recording functional.
3. LMU: staff launches from kiosk → game starts on pod. Timer billing active.
4. AC multiplayer: 2+ pods launch simultaneously, all laps recorded, billing atomic.

**Plans:** TBD

---

### Phase 390: Spectator Displays + Cloud Access

**Goal:** Leaderboards on spectator PCs and cloud dashboard accessible via public URL.
**Depends on:** Phase 384 (laps must be flowing for leaderboard to show data)
**Executor:** James (spectator PCs), Bono (cloud DNS + TLS)
**Requirements:** DISP-01, DISP-02, CLUD-01, CLUD-02

**Success criteria:**
1. ⬜ Leaderboard display on spectator PCs — needs James at venue
2. ⬜ Live circuit viewer on spectator PCs — needs James at venue
3. ✅ DNS A record: cloud.racingpoint.cloud → 72.60.101.58. TLS cert valid until 2026-07-09. HTTPS works.
4. ✅ Cloud dashboard online (PM2 `cloud-dashboard`, 36h uptime). Magic-link auth available.

**Cloud side complete.** Spectator PCs require James deploying to .200, .32, .84, .37.

---

### Phase 391: Digital Staff Operations — COMPLETE 2026-04-16

**Goal:** Replace paper checklists with a digital audit trail. Morning opening and evening closing checklists enforced and tracked.
**Depends on:** Phase 384 (P0 verified)
**Executor:** Bono
**Requirements:** CHKL-01, CHKL-02, CHKL-03

**Success criteria:**
1. ✅ `staff_checklists` + `staff_checklist_completions` tables. Per-item timestamps, staff_id, notes.
2. ✅ 2 seeded templates: Morning Opening (8 items), Evening Closing (7 items). Items configurable via JSON, admin can add custom checklists.
3. ✅ GET /staff/checklists/compliance — 7-day compliance view with completion % and gap detection.

**Commits:** `93d75f88`

---

### Phase 392: Unified Readiness Review

**Goal:** Full system verification before declaring v49 complete. Everything works end-to-end.
**Depends on:** All previous phases + v47.0 completion
**Executor:** Joint (Uday signs off)
**Requirements:** Cross-wave verification

**Success criteria:**
1. Lap recording: AC + F1 25 laps appear on leaderboard within 10s (re-verify since Phase 384)
2. Pricing engine: Bono adjusts a price, it appears in PWA within 1 sync cycle
3. Marketing: empty pod triggers offer to opted-in customer — verify WhatsApp delivery
4. Opt-out: customer sends "stop" — verify no more promos sent
5. Spectator: leaderboard visible on at least 2 spectator PCs
6. Cloud: cloud.racingpoint.cloud loads over HTTPS
7. All v47.0 phases either complete or explicitly deferred with reason
8. Uday signs off on venue-ready state

**Plans:** TBD

---

### Phase 392.1: P0 Zero-Laps 3-Layer Fix + Folded C1 FK-PRAGMA Deploy (INSERTED 2026-04-16)

**Goal:** Restore lap recording end-to-end on per-minute sessions. Root cause confirmed from 2026-04-16 Pod 8 jsonl log + DB probe: per-minute tier `min_duration_secs` ~60s < fastest F2004 Spa lap ~105s, so AC session ends before emitting any lap. DB evidence: `laps=0`, `lap_rejections=0` — events never reach the DB layer (not a rejection path). Folds the C1 FK-PRAGMA source fix (d24b17f7) into the same binary swap window since both require a racecontrol rebuild + server+cloud swap.
**Depends on:** Phase 383 (v48 deploy pipeline baseline)
**Executor:** James (venue), Bono (cloud)
**Requirements:** URGENT P0 — Uday #1 priority, 30 days on-site with zero laps recorded

**Scope — 3 layers:**

1. **Layer 1 (ship-now):** Raise per-minute tier `min_duration_secs` from ~60 → ≥120. Target store: `pricing_rules` table OR `billing_config.toml` (confirm which during planning). Files to audit: `billing_start.rs`, `billing_start_validate.rs`, `pricing_billing_rates.rs`, `billing_pricing.rs`. **DEPLOY PARITY:** server .23 + Bono VPS.
2. **Layer 2 (kiosk UX warning):** Tier selector shows *"Track X × Car Y needs ≥ N min for 1 lap. Your N-min session will not register a lap."* Requires per-track × per-car reference lap times — data source unknown, flagged NOT TESTED.
3. **Layer 3 (server grace window):** `billing_timer_expiry.rs` lap-aware grace — extend per-minute expiry by `fastest_lap × 1.5`. Requires reference lap data model + MMA audit (cross-system: billing touches wallet debit).

**Folded — C1 FK-PRAGMA deploy (d24b17f7):**

- Pre-start cleanup SQL (venue then cloud):
  1. `DELETE FROM billing_events WHERE billing_session_id NOT IN (SELECT id FROM billing_sessions);`
  2. `DELETE FROM billing_sessions WHERE pricing_tier_id NOT IN (SELECT id FROM pricing_tiers);`
- Deploy order: venue cleanup → venue swap → cloud cleanup → cloud swap. Cloud second so venue's FK-enforced writes can't push fresh orphans into cloud mid-window.
- Post-start verify: `PRAGMA foreign_keys = 1` via a SECOND pool connection (first conn may be cached).
- Orphan counts = 0 on both environments.
- Rollback snapshots captured 2026-04-16 ~04:18 UTC:
  - Venue: `C:/RacingPoint/backups/racecontrol-pre-c1-20260416.db` (176,910,336 B)
  - Cloud: `/root/racecontrol/backups/racecontrol-pre-c1-20260416.db` (172,019,712 B)

**Success criteria (6 checks):**

1. Staff runs 2-min per-minute session on Pod 8 with Brands Hatch Indy + road car
2. Row in `laps` table within 10s of session end
3. Authenticated `GET /api/v1/laps` from James .27 (NOT SSH curl on server) returns the row
4. `PRAGMA foreign_keys = 1` on server .23 AND Bono VPS via 2nd pool connection
5. Orphan counts = 0 on both environments
6. No regression in wallet/membership billing flows

**NOT TESTED (carry into execution):**

- Other FK-declaring tables (`billing_rates`, `wallet_transactions`, `drivers`, `laps`, `sessions`, `pricing_tiers`) — may have their own orphans we haven't swept
- Live UPDATE code paths touching deleted orphan rows — sqlx will now return FK errors instead of swallowing
- Per-track × per-car fastest-lap reference data source for Layer 2/3 — does it exist? where?

**OUT OF SCOPE (file as separate phases post-ship):**

- Strategy-B `launch_state_is_live` tracking bug (gate works by accident)
- Server ↔ agent `launch_id` protocol mismatch (server 43e35dc7 predates agent builds)
- Old-build `1136fd1a` exit-grace fleet rollout

**Plans:** TBD

---

### v49.0 Progress

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 383. Deploy & Verify Pipeline | 0/TBD | Cloud parity done (`c9c20ef3`), venue pending | - |
| 384. Lap Recording Wiring | 0/TBD | Awaiting venue AC launch test | - |
| 385. Architecture Completion | 1/1 | Complete (`f04efacb`) | 2026-04-16 |
| 386. Autonomous Pricing Engine | 1/1 | Complete (`8d4d0ce0`) | 2026-04-16 |
| 387. Customer Opt-In/Opt-Out | 1/1 | Complete (`f2bc7137`) | 2026-04-16 |
| 388. Autonomous Marketing Triggers | 1/1 | Complete (`09015ac1`) | 2026-04-16 |
| 389. Game Launch Completion | 0/TBD | Needs venue pod testing | - |
| 390. Spectator Displays + Cloud | 1/1 | Cloud done (DNS+TLS live), spectator PCs need James | 2026-04-16 |
| 391. Digital Staff Operations | 1/1 | Complete (`93d75f88`) | 2026-04-16 |
| 392. Unified Readiness Review | 0/TBD | Not started | - |
| 392.1. P0 Zero-Laps 3-Layer Fix + C1 FK-PRAGMA (INSERTED) | 0/1 | Layer 1 deployed, cloud FK verified + orphans cleaned, venue pending | - |

*v49.0 defined: 2026-04-14. Predecessor: v48.0 (10 phases code-committed, deploy pending).*
*Business context: ₹4.62L/month costs, 965 drivers, 75% one-time visitors, Pitlane competitor.*

### Phase 413: Service key provisioning + deploy-server.sh hardening (Option Z + respawn race fixes)

**Goal:** rc-agent Tier 0 oracle live fleet-wide; deploy-server.sh respawn race eliminated.
**Requirements**: TBD
**Depends on:** Phase 392.1 (prior active phase)
**Plans:** 8/11 plans executed

Plans:
- [x] 413-02-PLAN.md — rc-agent mesh_key_cache module (Option Z data layer): MeshKeyCache type + fetch_from_server + get_key_or_env with W5 403-warn observability; 10 unit tests passing. Commits `45d85c14` (module+deps, labeled 413-01 due to parallel-agent commit collision) + `85b1968e` (mod declaration in main.rs — no lib.rs deviation documented)
- [x] 413-03-PLAN.md — rc-agent MeshKeyCache boot wire-up: main.rs instantiates cache + initial fetch + spawn_periodic_refetch at 300s interval. Commit `28de9e30`. 10 mesh_key_cache tests still green; release build clean.
- [x] 413-04-PLAN.md — rc-agent MeshKeyCache consumer rewire (3 env-readers → cache-first): ai_debugger::check_audit_known_issues + remote_ops::require_service_key (W4 Option (a) sub-router with State<MeshKeyCache>) + ws_handler csv_lap_fallback. W5 extends 403-warn to Tier 0. S10 new test_service_key_cache_wins_over_env. AppState.mesh_key_cache field. 103 tests passing incl. all 7 legacy service-key tests. Production env reads: 3 → 0 in http-client builds. Commits `51356322` (Tasks 1+2+scaff) + `34e13516` (Task 3). Closes Gap 4 structurally.
- [x] 413-05-PLAN.md — deploy-server.sh Factor 1: schtasks disable/re-enable coverage extended from 2 → 8 tasks (commits `0fc38726`, `e38a9e81`, `7c7af7ec`) — shipped 2026-04-17 IST as script-only change; first exercise on next `bash scripts/deploy-server.sh` run
- [x] 413-06-PLAN.md — deploy-server.sh Factor 2: deploy sentinel renamed from DEPLOY_IN_PROGRESS → OTA_DEPLOYING in all 3 blocks. Commit `d92c3843`. Writer + checker now agree on sentinel name; PS watchdog will skip its restart during kill→swap→start window.

---

## v52.0 Claude Workspace Restructure

**Started:** 2026-04-15
**Goal:** Consolidate ~14 classes of scattered Claude-side artifacts across 8+ locations into a single canonical workspace repo with deterministic James↔Bono sync verified by `cgp-distribution-probe.js`.
**Source:** `memory/project_workspace_restructure.md`
**Requirements:** `.planning/REQUIREMENTS-v52.md`

**Core gate (applies to EVERY phase):**
> `node workspace/scripts/cgp-distribution-probe.js` must show 100% parity on cross-platform hooks before phase close. Single binary gate between "phase done" and "phase not done."

### Success Criteria

1. Fresh machine onboarded by `git clone workspace && bash workspace/sync/bootstrap-new-machine.sh` — no manual file copying
2. `ARCHITECTURE.md` + `CONVENTIONS.md` answer "where does a new X go?" for any X
3. James hooks and Bono VPS hooks are byte-identical on cross-platform files (probe confirms)
4. No Claude session needs to ask "where should I put this script?"
5. Drift cannot accumulate silently because `verify-parity.sh` runs pre-commit

### Phases (393 → 412)

Restructured 2026-04-16 based on decisions locked in Phase 393 (8 foundation decisions) and Phase 394 (superset-wins canonical files + deferred drift). Key changes from initial 16-phase plan: split FND-02 (drift) across 394/395, split FND-04 (skeleton) across 397/398, added MIG-04 (secrets boundary per D-6) and MIG-05 (agents+commands per D-8), broke out Uday repo-creation gate + CI workflow as explicit phase 397. Net: +4 phases (20 total).

| # | Phase | Reqs | Goal |
|---|---|---|---|
| 393 | Foundation Decisions | FND-01 | 8 decisions locked: repo (`workspace` under Uday GitHub), layout (typed folders), branch model (main + wip/*, squash-merge, 24h GC), install model (copy not symlink), CI gate (6 checks), secrets (`~/.claude-secrets/` NEW), session state (outside repo), agents+commands (join workspace) |
| 394 | Resolve CGP Drift (superset files) | FND-02a | Canonicalize `cgp-enforce.js` + `cgp-session-inject.js` per-hunk via James superset-wins; memory-only, no disk writes. **✓ COMPLETE 2026-04-15** |
| 395 | Resolve Remaining Hook Drift + Classify Single-Machine Hooks | FND-02b | Canonicalize 6 deferred drifted files (gsd-check-update, gsd-context-monitor, gsd-prompt-guard, gsd-statusline, gsd-workflow-guard, memory-staleness-check); classify 16 James-only + 4 Bono-only hooks into cross-platform / windows-only / linux-only buckets. Produces the manifest Phase 404 install.sh consumes. |
| 396 | Architecture + Conventions Docs | FND-03 | Formalize `ARCHITECTURE.md` + `CONVENTIONS.md` from 393 drafts. Every rule names its mechanical enforcer or gets deleted. **✓ COMPLETE 2026-04-16** |
| 397 | Uday Repo Gate + CI Workflow + Pre-commit | FND-04a | **HUMAN GATE:** verify Uday created `workspace` repo under his GitHub account + added `james-racingpoint` + `bono-racingpoint` as push collaborators. Then write `.github/workflows/ci.yml` (6 checks from D-5) and `sync/pre-commit` (secret scan blocklist). |
| 398 | Init Workspace Skeleton | FND-04b | Clone fresh `workspace` repo; write `.gitignore` (.env, secrets, session state); commit skeleton; run `cgp-distribution-probe.js` from skeleton → must be green on empty state before any migration. |
| 399 | Migrate Scripts/Probes | MIG-01 | Move `memory/scripts/cgp-distribution-probe.js` + `openrouter-key-recovery.js` → `workspace/scripts/`; grep-update every reader. |
| 400 | Migrate Memory + Create memory/INDEX.md | MIG-02 | Dry-run branch; move `memory/*.md` → `workspace/memory/`; create `memory/INDEX.md` (CI check #6 enforces orphan-free); update auto-memory path in global CLAUDE.md in same commit. |
| 401 | Secrets Boundary Migration → ~/.claude-secrets/ | MIG-04 | Per D-6: move `comms-link.env`, OpenRouter keys, PSK, relay keys from `~/.claude/` into `~/.claude-secrets/` on BOTH James and Bono; grep-update every reader; verify `.gitignore` + pre-commit blocklist prevent re-drift. |
| 402 | Migrate Agents + Slash Commands | MIG-05 | Per D-8: move `~/.claude/agents/` → `workspace/agents/` and `~/.claude/commands/` → `workspace/commands/`; update install.sh manifest to cover both. |
| 403 | Hook Tests Fixtures | MIG-03 | Per-hook fixtures in `workspace/tests/`: pre-flight-file-read 4-case, g9-auto-detect, backlog-enforce, cgp-enforce, cgp-session-inject. Built from 394+395 canonical text. |
| 404 | Sync Tooling: install.sh + verify-parity.sh | HOOK-01 | Consumes 395 classification manifest. Idempotent copy from workspace → `~/.claude/`. Tests on Git Bash (Windows) AND bash (Linux). Triggers from post-merge git hook. |
| 405 | Hooks Migration — James | HOOK-02 | Backup `~/.claude/hooks/` → `.backup-v52/`; run `sync/install.sh` from workspace; probe 100% parity on cross-platform. |
| 406 | Hooks Migration — Bono + Offline Bare Mirror | HOOK-03 | Bono backs up own hooks, pulls workspace, runs install.sh. Set up `bono-vps:/root/workspace-mirror.git` bare mirror + post-receive hook per D-1 offline fallback. |
| 407 | Parity Verification Gate | HOOK-04 | Cross-machine probe run: James + Bono + fresh bootstrap clone all show 100% parity on cross-platform hooks. **THIS PHASE IS THE SYNC PROOF.** |
| 408 | Settings Migration | CLN-01 | Extract shared `workspace/settings/base.json`; per-machine `settings.local.json`; `install-settings.sh` merge logic that doesn't clobber local overrides. |
| 409 | Bootstrap Consolidation | CLN-02 | Move `claude-code-bootstrap/{vps,windows}/` → `workspace/bootstrap/`. Update onboarding docs. (Agents/commands already migrated in 402 — this phase is JUST bootstrap scripts.) |
| 410 | Protocol Doc Pointers | CLN-03 | Decide pointer vs cached copy for CGP.md / MMA.md; update all CLAUDE.md references to canonical workspace location. |
| 411 | Decommission Old Paths | CLN-04 | Remove `claude-code-bootstrap/`; archive old memory git history as read-only tag; update `docs/ARCHITECTURE.md`. |
| 412 | Milestone Close | CLN-05 | Final parity audit across James + Bono + fresh bootstrap; `MIGRATION-LOG.md`; Bono sign-off via comms-link; close v52.0. |

### Phase Details (393 → 412)

> Headings below mirror the table above for `gsd-tools` parser compatibility. Goal text is the source of truth in the table.

#### Phase 393: Foundation Decisions

**Goal:** 8 decisions locked — repo, layout, branch model, install model, CI gate, secrets, session state, agents+commands. FND-01.

#### Phase 394: Resolve CGP Drift (superset files)

**Goal:** Canonicalize `cgp-enforce.js` + `cgp-session-inject.js` per-hunk via James superset-wins. FND-02a. ✓ COMPLETE 2026-04-15.

#### Phase 395: Resolve Remaining Hook Drift + Classify Single-Machine Hooks

**Goal:** Canonicalize 6 deferred drifted files + classify 16 James-only + 4 Bono-only hooks. Produces Phase 404 install manifest. FND-02b.

#### Phase 396: Architecture + Conventions Docs

**Goal:** Formalize `ARCHITECTURE.md` + `CONVENTIONS.md` from 393 drafts. Every convention names its mechanical enforcer or gets deleted. FND-03.

#### Phase 397: Uday Repo Gate + CI Workflow + Pre-commit

**Goal:** HUMAN GATE — Uday creates `workspace` repo + adds collaborators. Write `.github/workflows/ci.yml` (6 checks) + `sync/pre-commit` secret scan. FND-04a.

#### Phase 398: Init Workspace Skeleton

**Goal:** Clone fresh `workspace` repo; write `.gitignore`; commit skeleton; run `cgp-distribution-probe.js` green on empty state before migration. FND-04b.

#### Phase 399: Migrate Scripts/Probes

**Goal:** Move `memory/scripts/cgp-distribution-probe.js` + `openrouter-key-recovery.js` → `workspace/scripts/`; grep-update every reader. MIG-01.

#### Phase 400: Migrate Memory + Create memory/INDEX.md

**Goal:** Dry-run branch; move `memory/*.md` → `workspace/memory/`; create `memory/INDEX.md` (CI check #6 orphan-free); update auto-memory path. MIG-02.

#### Phase 401: Secrets Boundary Migration → ~/.claude-secrets/

**Goal:** Per D-6 — move `comms-link.env`, OpenRouter keys, PSK, relay keys from `~/.claude/` into `~/.claude-secrets/` on BOTH James and Bono. MIG-04.

#### Phase 402: Migrate Agents + Slash Commands

**Goal:** Per D-8 — move `~/.claude/agents/` → `workspace/agents/` and `~/.claude/commands/` → `workspace/commands/`; update install.sh manifest. MIG-05.

#### Phase 403: Hook Tests Fixtures

**Goal:** Per-hook fixtures in `workspace/tests/`: pre-flight-file-read, g9-auto-detect, backlog-enforce, cgp-enforce, cgp-session-inject. Built from 394+395 canonical text. MIG-03.

#### Phase 404: Sync Tooling: install.sh + verify-parity.sh

**Goal:** Consumes 395 classification manifest. Idempotent copy workspace → `~/.claude/`. Tests on Git Bash + bash. Triggers from post-merge git hook. HOOK-01.

#### Phase 405: Hooks Migration — James

**Goal:** Backup `~/.claude/hooks/` → `.backup-v52/`; run `sync/install.sh` from workspace; probe 100% parity on cross-platform. HOOK-02.

#### Phase 406: Hooks Migration — Bono + Offline Bare Mirror

**Goal:** Bono backs up hooks, pulls workspace, runs install.sh. Set up `bono-vps:/root/workspace-mirror.git` bare mirror + post-receive hook per D-1. HOOK-03.

#### Phase 407: Parity Verification Gate

**Goal:** Cross-machine probe: James + Bono + fresh bootstrap clone all show 100% parity on cross-platform hooks. **THIS PHASE IS THE SYNC PROOF.** HOOK-04.

#### Phase 408: Settings Migration

**Goal:** Extract shared `workspace/settings/base.json`; per-machine `settings.local.json`; `install-settings.sh` merge logic that doesn't clobber local overrides. CLN-01.

#### Phase 409: Bootstrap Consolidation

**Goal:** Move `claude-code-bootstrap/{vps,windows}/` → `workspace/bootstrap/`. Update onboarding docs. (Agents/commands already migrated in 402.) CLN-02.

#### Phase 410: Protocol Doc Pointers

**Goal:** Decide pointer vs cached copy for CGP.md / MMA.md; update all CLAUDE.md references to canonical workspace location. CLN-03.

#### Phase 411: Decommission Old Paths

**Goal:** Remove `claude-code-bootstrap/`; archive old memory git history as read-only tag; update `docs/ARCHITECTURE.md`. CLN-04.

#### Phase 412: Milestone Close

**Goal:** Final parity audit across James + Bono + fresh bootstrap; `MIGRATION-LOG.md`; Bono sign-off via comms-link; close v52.0. CLN-05.

### Session Discipline

**ONE phase per session, maximum.** Each session reads `memory/project_workspace_restructure.md` for context, reads the phase's PLAN.md for immediate work, executes, runs the probe, updates SUMMARY.md, writes a handoff naming the next phase's entry conditions, stops. Do not stack phases.

### Hard Blockers

- **Phase 397 is a human gate:** Uday must create the `workspace` repo + add collaborators before 397 can close. 398+ blocked until 397 green.
- **Phases 405-406 require Bono ratification** of the Phase 393 decisions (currently pending via comms-link INBOX `68d453f`). Without ratification, 405 (James hook migration) may proceed in read-only mode but 406 (Bono hook migration) cannot start.

### Bono Coordination

Before any hook migration (405+), Bono needs to (1) review canonical decisions on drifted files from 394+395, (2) acknowledge sync contract + decisions from 393, (3) stage backup of its current hook set. Mechanism: decision doc committed to memory (auto-pushes to Bono backup) + explicit comms-link ratification message before 405 kickoff.

### v52.0 Progress

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 393. Foundation Decisions | 0/0 | Locked (awaiting Bono ratification) | 2026-04-15 (draft) |
| 394. Resolve CGP Drift (superset files) | 1/1 | ✓ Complete | 2026-04-15 |
| 395. Resolve Remaining Hook Drift + Classify | 1/1 | ✓ Complete | 2026-04-16 |
| 396. Architecture + Conventions Docs | 1/1 | ✓ Complete | 2026-04-16 |
| 397. Uday Repo Gate + CI Workflow + Pre-commit | 0/TBD | Blocked on Uday repo creation | - |
| 398. Init Workspace Skeleton | 0/TBD | Blocked on 397 | - |
| 399. Migrate Scripts/Probes | 0/TBD | Not started | - |
| 400. Migrate Memory + INDEX.md | 0/TBD | Not started | - |
| 401. Secrets Boundary Migration | 0/TBD | Not started | - |
| 402. Migrate Agents + Slash Commands | 0/TBD | Not started | - |
| 403. Hook Tests Fixtures | 0/TBD | Not started | - |
| 404. Sync Tooling | 0/TBD | Not started | - |
| 405. Hooks Migration — James | 0/TBD | Blocked on Bono ratification | - |
| 406. Hooks Migration — Bono + Bare Mirror | 0/TBD | Blocked on 405 + Bono ratification | - |
| 407. Parity Verification Gate | 0/TBD | Not started | - |
| 408. Settings Migration | 0/TBD | Not started | - |
| 409. Bootstrap Consolidation | 0/TBD | Not started | - |
| 410. Protocol Doc Pointers | 0/TBD | Not started | - |
| 411. Decommission Old Paths | 0/TBD | Not started | - |
| 412. Milestone Close | 0/TBD | Not started | - |

*v52.0 defined: 2026-04-15. Restructured 2026-04-16 (Option A: +4 phases for secrets/agents/repo-gate/drift-remainder). Parallel to v49.0 (not blocking). Core gate: `cgp-distribution-probe.js` 100% parity on cross-platform hooks.*

### Phase 414: Continuous Billing Session (Option 1 + Idle Auto-End)

**Goal:** Decouple billing-session lifetime from individual game lifetime so a customer can swap games/cars/tracks freely inside one paid session. Meter only ticks while game is `Running` and driver is `Active`. After 15 min of no game running, auto-end with 10-min warning. Cumulative snap pricing across game swaps (15min AC + 15min F1 25 = ₹700 snap, not ₹750 per-minute).

**Requirements**:
- BE: New `BillingEvent::GameStopped`; FSM transitions Active→WaitingForGame, WaitingForGame→{Completed,EndedEarly}; `between_games_idle_seconds` field on BillingTimer; tick semantics for WaitingForGame (10-min IdleWarning broadcast, 15-min auto-end).
- BE: When game state transitions Running→Stopped/Crashed inside an Active billing session, fire GameStopped event (status → WaitingForGame, meter pauses).
- BE: New `DashboardEvent::IdleWarning { pod_id, session_id, balance_paise, seconds_remaining }` (server→dashboard only — no rc-agent protocol churn).
- FE: Kiosk staff page when `status==WaitingForGame && elapsed_seconds>0` shows "Continue with another game" + "End session" buttons + paused-meter UI with cumulative cost.
- FE: IdleWarning modal with balance check; "insufficient balance to continue" branch when wallet < 1 min worth.
- TESTS: snap-across-swap (25min+5min=₹700), idle auto-end at 900s, warning at 600s, idle-counter reset on resume, FSM transitions valid, balance-insufficient at warning, End/EndEarly from WaitingForGame.

**Depends on:** Phase 413 (no hard dep — independent feature; sequencing only). Design contract at `~/.claude/projects/C--Users-bono/memory/decision_billing_continuous_session_design.md`.

**Open risks (resolve in plan-phase):**
1. Existing `WaitingForGame` consumers may assume `elapsed_seconds == 0` — needs grep audit.
2. FSM today rejects `EndEarly` from `WaitingForGame` (per `api/billing_session.rs:259`) — must add transition.
3. Activity log noise on every game swap.
4. Crash vs clean-stop: 10-min PausedCrashRecovery vs 15-min WaitingForGame — two timeouts, valid?
5. Server restart mid-WaitingForGame → idle counter resets (customer-favourable but undocumented).
6. Balance gate threshold definition (< 1 min @ ₹25/min = ₹25 floor?).

**Plans:** 0 plans

Plans:
- [ ] TBD (run /gsd:plan-phase 414 to break down)

---

## v50.0 — rc-agent-mobile (Reception Automation Hub) — Phases 429–444

**Started:** 2026-04-18 (Planning phase)
**Status:** Kickoff-ready. Phase 429 plan pending.
**Requirements:** `.planning/REQUIREMENTS-v50.md` (54 requirements across 14 categories)
**Roadmap detail:** `.planning/ROADMAP-v50.md` (full phase breakdown, dependency graph, ship-gate checklist)
**Source spec:** `~/.claude/projects/C--Users-bono/memory/project_v50_rc_agent_mobile.md`

**Goal:** Kotlin Android agent on 1× Lenovo Tab Plus + 1× Samsung Galaxy M07 automating reception-workflow apps (Zomato Partner P1, HyperPure P2, Blinkit P3, cardboard vendor P4-deferred) via Accessibility Service. Registers with comms-link relay like existing Windows rc-agent.

**10 extensibility features (non-negotiable for future-proofing):** pluggable driver framework, selector DSL + hot-reload, capability registry, credential abstraction, protocol versioning, per-device + per-driver feature flags, humanize layer, audit log, remote selector push, multi-device-type readiness.

### Phases

### Phase 429: Kotlin Scaffold + HTTP Server + Comms-link Registration
**Goal:** Agent installs on Tab Plus + M07, runs Foreground Service, exposes local HTTP endpoints, registers with comms-link, sends heartbeat, survives reboot.
**Requirements:** AGENT-01..08

### Phase 430: Accessibility Service Foundation
**Goal:** Agent reads screen-tree and dispatches tap/swipe/text on any foreground app.
**Requirements:** ACCESS-01..05

### Phase 431: Bootstrap Install + First-run UX
**Goal:** Non-technical staff installs agent via MTP sideload + Files app, completes first-run permissions in < 5min.
**Requirements:** INSTALL-01..03

### Phase 432: Driver Framework + Capability Registry
**Goal:** Drivers are plugins registered via manifest; device declares supported driver types; failures isolated.
**Requirements:** DRIVER-01..05, CAPREG-01..04

### Phase 433: Selector DSL + Hot-Reload
**Goal:** YAML selectors are source of truth; hot-reload within 10s; versioned per app version; fallback chain.
**Requirements:** SELECTOR-01..06

### Phase 434: Credential Abstraction
**Goal:** `CredentialStrategy` interface with `PersistentSession` impl; OTP/OAuth slots ready.
**Requirements:** CRED-01..04

### Phase 435: Humanize Layer + Audit Log
**Goal:** All UI actions pass through humanize interceptor (delays, business-hours, rate limit) and emit audit events with screenshot hash.
**Requirements:** HUMANIZE-01..04, AUDIT-01..04

### Phase 436: Feature Flag System
**Goal:** Server-side per-device + per-driver flags push-sync to agent within 10s; kill-switch halts all drivers fleet-wide.
**Requirements:** FLAG-01..04

### Phase 437: Zomato Partner Driver (P1)
**Goal:** Auto-accept (capacity-gated) / auto-reject / mark-ready Zomato orders; WhatsApp + Discord forwarding; session-expiry alerting.
**Requirements:** ZOMATO-01..06

### Phase 438: HyperPure Driver (P2)
**Goal:** Accept bulk order from Core inventory trigger, navigate HyperPure app, check out, log confirmation.
**Requirements:** HYPER-01..05

### Phase 439: Blinkit Driver (P3)
**Goal:** Accept emergency top-up from staff trigger; navigate Blinkit; log order + ETA.
**Requirements:** BLINK-01..04

### Phase 440: Cardboard Vendor Driver (P4, deferred)
**Goal:** Drop-in driver when vendor app is identified (open question Q2). Auto-skips ship gate if Q2 unresolved.
**Requirements:** CARDBOARD-01..02

### Phase 441: Admin Dashboard Reception View
**Goal:** Unified reception page in admin dashboard showing orders/deliveries/device status.
**Requirements:** ADMIN-01..03

### Phase 442: Feature Flag + Capability UI
**Goal:** Admin toggles driver enablement and views per-device capability list.
**Requirements:** ADMIN-04, FLAG-01..04

### Phase 443: Selector-Map Remote Push UI
**Goal:** Admin uploads signed selector YAML, targets devices, rolls back on failure.
**Requirements:** ADMIN-05, SELECTOR-04

### Phase 444: E2E Drills + ToS Incident Playbook
**Goal:** All failure paths drilled end-to-end; ToS-incident runbook documented.
**Requirements:** E2E-01..04, ADMIN-06

### v50.0 Progress

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 429. Kotlin Scaffold + HTTP + Registration | 0/TBD | Ready to plan | - |
| 430. Accessibility Service Foundation | 0/TBD | Blocked on 429 | - |
| 431. Bootstrap Install + First-run UX | 0/TBD | Blocked on 429 | - |
| 432. Driver Framework + Capability Registry | 0/TBD | Blocked on 430 | - |
| 433. Selector DSL + Hot-Reload | 0/TBD | Blocked on 432 | - |
| 434. Credential Abstraction | 0/TBD | Blocked on 432 | - |
| 435. Humanize Layer + Audit Log | 0/TBD | Blocked on 432 | - |
| 436. Feature Flag System | 0/TBD | Blocked on 432 | - |
| 437. Zomato Partner Driver (P1) | 0/TBD | Blocked on 433,434,435,436 | - |
| 438. HyperPure Driver (P2) | 0/TBD | Blocked on 437 | - |
| 439. Blinkit Driver (P3) | 0/TBD | Blocked on 437 | - |
| 440. Cardboard Vendor Driver (deferred) | 0/TBD | Blocked on 437 + vendor app | - |
| 441. Admin Dashboard Reception View | 0/TBD | Blocked on 437 | - |
| 442. Feature Flag + Capability UI | 0/TBD | Blocked on 436,441 | - |
| 443. Selector-Map Remote Push UI | 0/TBD | Blocked on 433,441 | - |
| 444. E2E Drills + ToS Playbook | 0/TBD | Blocked on 437,438,439,441 | - |

*v50.0 defined: 2026-04-18. Greenfield Kotlin/Android project; shared JSON protocol with Rust rc-agent, no shared code. Runs parallel to v48/v49/v52 (not blocking).*
