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

See `.planning/milestones/` for archived roadmaps and requirements per milestone.

---

## v38.0 Security Hardening & Operational Maturity

**Goal:** Harden the security posture — venue CA with mTLS, JWT rotation, hash-chained audit logs, RBAC, and automated security scanning.

**Phases:** 5  |  **Coverage:** 19/19 requirements mapped

**Dependency graph:**
```
305 (TLS) ──┬──> 306 (WS Auth) ──> 308 (RBAC) ──┐
            └──> 307 (Audit Chain) ───────────────┴──> 309 (Security Audit)
```

### Phases

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
**Plans:** 2/2 plans complete

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

*Last updated: 2026-04-02 after v37+v38 merge reconciliation*
