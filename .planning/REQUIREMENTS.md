# Requirements: v37.0 Data Durability & Multi-Venue Readiness

**Defined:** 2026-04-01
**Core Value:** Ensure operational data survives hardware failure and prepare the data layer for a potential second venue

## v1 Requirements

### Backup Pipeline (BACKUP)

- [x] **BACKUP-01**: Server performs hourly SQLite .backup (WAL-safe) of all operational databases
- [x] **BACKUP-02**: Local backup rotation retains 7 daily + 4 weekly snapshots, auto-purging older files
- [x] **BACKUP-03**: Nightly backup is SCP'd to Bono VPS with integrity verification (SHA256 match)
- [x] **BACKUP-04**: WhatsApp alert fires if newest backup is older than 2 hours (staleness detection)
- [x] **BACKUP-05**: Backup status visible in admin dashboard (last backup time, size, destination health)

### Cloud Data Sync (SYNC)

- [x] **SYNC-01**: cloud_sync.rs syncs fleet_solutions table to Bono VPS (server-authoritative)
- [x] **SYNC-02**: cloud_sync.rs syncs model_evaluations table to Bono VPS (server-authoritative)
- [x] **SYNC-03**: cloud_sync.rs syncs metrics_rollups table to Bono VPS (server-authoritative)
- [x] **SYNC-04**: Cloud is authoritative for cross-venue data (future venue 2 solutions flow back)
- [x] **SYNC-05**: Sync handles conflicts gracefully — last-write-wins with venue_id tiebreaker
- [x] **SYNC-06**: Sync status visible in admin dashboard (last sync time, tables synced, conflict count)

### Event Archive (EVENT)

- [x] **EVENT-01**: All significant events written to SQLite events table with structured schema (type, source, pod, timestamp, payload)
- [x] **EVENT-02**: Daily JSONL export of events table for archival
- [x] **EVENT-03**: SQLite events retained for 90 days, then purged (JSONL is permanent archive)
- [x] **EVENT-04**: Nightly JSONL files shipped to Bono VPS via SCP
- [x] **EVENT-05**: Events queryable via REST API (GET /api/v1/events with filters: type, pod, date range)

### Multi-Venue Schema (VENUE)

- [x] **VENUE-01**: All major tables have venue_id column (default: 'racingpoint-hyd-001')
- [x] **VENUE-02**: Migration is backward compatible — existing data gets default venue_id, no functional change
- [ ] **VENUE-03**: All INSERT/UPDATE queries include venue_id (prepared for multi-venue)
- [x] **VENUE-04**: Design doc created: MULTI-VENUE-ARCHITECTURE.md with trigger conditions for venue 2

### Fleet Deploy Automation (DEPLOY)

- [ ] **DEPLOY-01**: POST /api/v1/fleet/deploy endpoint accepts binary hash + target scope (all/canary/specific pods)
- [ ] **DEPLOY-02**: Canary deploy to Pod 8 first, health verify before fleet rollout
- [ ] **DEPLOY-03**: Auto-rollout to remaining pods after canary passes (configurable delay between waves)
- [ ] **DEPLOY-04**: Auto-rollback on failure — if canary or any wave fails health check, revert to previous binary
- [ ] **DEPLOY-05**: Deploy status endpoint (GET /api/v1/fleet/deploy/status) shows progress, wave status, rollback events
- [ ] **DEPLOY-06**: Active billing sessions drain before binary swap on each pod (existing OTA sentinel protocol)

## v2 Requirements

### Advanced Backup

- **BACKUP-V2-01**: Point-in-time recovery from backup + WAL replay
- **BACKUP-V2-02**: Encrypted backups at rest on Bono VPS

### Advanced Sync

- **SYNC-V2-01**: Bidirectional sync with conflict resolution UI in admin
- **SYNC-V2-02**: Real-time sync via WebSocket (not periodic)

## Out of Scope

| Feature | Reason |
|---------|--------|
| PostgreSQL migration | SQLite WAL sufficient for single venue; trigger: venue 2 confirmed |
| MinIO/S3 object storage | Local backup + SCP sufficient; trigger: artifacts > 50GB |
| Kubernetes deployment | Windows pods, never applicable |
| Multi-server racecontrol | Single server sufficient for 8 pods; trigger: multi-server confirmed |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| BACKUP-01 | Phase 300 | Complete |
| BACKUP-02 | Phase 300 | Complete |
| BACKUP-03 | Phase 300 | Complete |
| BACKUP-04 | Phase 300 | Complete |
| BACKUP-05 | Phase 300 | Complete |
| SYNC-01 | Phase 301 | Complete |
| SYNC-02 | Phase 301 | Complete |
| SYNC-03 | Phase 301 | Complete |
| SYNC-04 | Phase 301 | Complete |
| SYNC-05 | Phase 301 | Complete |
| SYNC-06 | Phase 301 | Complete |
| EVENT-01 | Phase 302 | Complete |
| EVENT-02 | Phase 302 | Complete |
| EVENT-03 | Phase 302 | Complete |
| EVENT-04 | Phase 302 | Complete |
| EVENT-05 | Phase 302 | Complete |
| VENUE-01 | Phase 303 | Complete |
| VENUE-02 | Phase 303 | Complete |
| VENUE-03 | Phase 303 | Pending |
| VENUE-04 | Phase 303 | Complete |
| DEPLOY-01 | Phase 304 | Pending |
| DEPLOY-02 | Phase 304 | Pending |
| DEPLOY-03 | Phase 304 | Pending |
| DEPLOY-04 | Phase 304 | Pending |
| DEPLOY-05 | Phase 304 | Pending |
| DEPLOY-06 | Phase 304 | Pending |

**Coverage:**
- v1 requirements: 27 total
- Mapped to phases: 27
- Unmapped: 0

---
*Requirements defined: 2026-04-01*
*Last updated: 2026-04-01 — traceability completed during roadmap creation*
