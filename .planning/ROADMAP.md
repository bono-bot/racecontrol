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
- 🔨 **v41.0 Game Intelligence System** — Phases 315+

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
- [ ] **Phase 313: Game State Resilience** — GSTATE-01, GSTATE-02, GSTATE-03
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
- [ ] 313-01-PLAN.md -- GSTATE-01/02/03: Launching hard-cap timeout, smart WS reconciliation, stop ACK cleanup

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
- [ ] **Phase 317: Server Inventory & Fleet Intelligence** — INV-02, COMBO-03, COMBO-04, LAUNCH-03, LAUNCH-04
- [x] **Phase 318: Launch Intelligence** — LAUNCH-01, LAUNCH-05 (completed 2026-04-03)
- [ ] **Phase 319: Reliability Dashboard** — DASH-01, DASH-02, DASH-03
- [ ] **Phase 320: Kiosk Game Filtering** — INV-03, COMBO-05

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
- [ ] 317-01-PLAN.md -- game_inventory.rs (pod_game_inventory + combo_validation_flags tables, upsert fns, fleet_validity, auto-disable), WS handlers for GameInventoryUpdate + ComboValidationReport, fleet_validity in GET /api/v1/presets
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
- [ ] 319-01-PLAN.md -- Fleet game matrix (GET /api/v1/fleet/game-matrix from pod_game_inventory) + combo reliability table (GET /api/v1/admin/combo-list from combo_reliability, sortable, red highlight < 70%) added to /games/reliability page
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
**Plans**: TBD

Plans:
- [ ] 320-01-PLAN.md -- Kiosk game filter: read installed_games from pod heartbeat WS state, filter GAME_DISPLAY list client-side; show unavailable badge on flagged AC combos; debounce inventory updates; UI-REVIEW.md gate before ship

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
**Plans:** 1/2 plans executed

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
