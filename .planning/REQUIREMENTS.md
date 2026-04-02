# Requirements: v38.0 Security Hardening & Operational Maturity

**Defined:** 2026-04-01
**Core Value:** Harden the attack surface after all data flows are established

## TLS for Internal HTTP (TLS)

- [ ] **TLS-01**: Self-signed venue CA generated via `scripts/generate-venue-ca.sh` with server + per-pod client certificates
- [ ] **TLS-02**: Axum server (:8080) accepts mTLS connections — rejects clients without valid venue CA cert
- [ ] **TLS-03**: rc-agent (:8090) accepts mTLS connections from server — rejects unauthorized callers
- [ ] **TLS-04**: Tailscale remote connections bypass mTLS (already encrypted) — mTLS is LAN-only

## WS Auth Hardening (WSAUTH)

- [x] **WSAUTH-01**: Per-pod JWT tokens with 24-hour expiry replace static PSK for WebSocket authentication
- [x] **WSAUTH-02**: JWT tokens auto-rotate 1 hour before expiry — zero-downtime refresh
- [x] **WSAUTH-03**: Invalid or expired JWT on WebSocket causes immediate disconnect + WhatsApp alert to staff
- [x] **WSAUTH-04**: PSK remains as bootstrap fallback — initial connection uses PSK, server issues JWT for subsequent auth

## Audit Log Integrity (AUDIT)

- [x] **AUDIT-01**: Every activity_log entry includes a SHA-256 hash linking to the previous entry (append-only chain)
- [x] **AUDIT-02**: Tamper detection: if any entry's previous_hash doesn't match the actual previous entry's hash, alert fires
- [x] **AUDIT-03**: Hash chain covers config changes, deploys, billing events, and admin actions
- [x] **AUDIT-04**: `GET /api/v1/audit/verify` endpoint returns chain integrity status (valid/broken + first broken entry)

## Role-Based Access Control (RBAC)

- [x] **RBAC-01**: Three roles defined: cashier (billing only), manager (billing + config), superadmin (everything)
- [x] **RBAC-02**: JWT tokens include role claim — server extracts and enforces on every protected endpoint
- [x] **RBAC-03**: Cashier role can only access billing, customer, and cafe endpoints — config/deploy/admin returns 403
- [x] **RBAC-04**: Admin dashboard UI shows/hides sections based on role — but enforcement is server-side (UI is convenience)

- [x] **VENUE-01**: All major tables have venue_id column (default: 'racingpoint-hyd-001')
- [x] **VENUE-02**: Migration is backward compatible — existing data gets default venue_id, no functional change
- [x] **VENUE-03**: All INSERT/UPDATE queries include venue_id (prepared for multi-venue)
- [x] **VENUE-04**: Design doc created: MULTI-VENUE-ARCHITECTURE.md with trigger conditions for venue 2

### Fleet Deploy Automation (DEPLOY)

- [x] **DEPLOY-01**: POST /api/v1/fleet/deploy endpoint accepts binary hash + target scope (all/canary/specific pods)
- [x] **DEPLOY-02**: Canary deploy to Pod 8 first, health verify before fleet rollout
- [x] **DEPLOY-03**: Auto-rollout to remaining pods after canary passes (configurable delay between waves)
- [x] **DEPLOY-04**: Auto-rollback on failure — if canary or any wave fails health check, revert to previous binary
- [x] **DEPLOY-05**: Deploy status endpoint (GET /api/v1/fleet/deploy/status) shows progress, wave status, rollback events
- [x] **DEPLOY-06**: Active billing sessions drain before binary swap on each pod (existing OTA sentinel protocol)

## Security Audit Script (SECAUDIT)

- [x] **SECAUDIT-01**: `scripts/security-audit.sh` scans: open ports, TLS config, JWT validity, default credentials, chain integrity
- [x] **SECAUDIT-02**: Output is structured JSON scorecard with pass/fail per check and overall score
- [x] **SECAUDIT-03**: Script integrates with existing `gate-check.sh` as a pre-deploy security gate

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
| VENUE-03 | Phase 303 | Complete |
| VENUE-04 | Phase 303 | Complete |
| DEPLOY-01 | Phase 304 | Complete |
| DEPLOY-02 | Phase 304 | Complete |
| DEPLOY-03 | Phase 304 | Complete |
| DEPLOY-04 | Phase 304 | Complete |
| DEPLOY-05 | Phase 304 | Complete |
| DEPLOY-06 | Phase 304 | Complete |
| TLS-01 | Phase 305 | Pending |
| TLS-02 | Phase 305 | Pending |
| TLS-03 | Phase 305 | Pending |
| TLS-04 | Phase 305 | Pending |
| WSAUTH-01 | Phase 306 | Done (b33e388e) |
| WSAUTH-02 | Phase 306 | Done (b33e388e) |
| WSAUTH-03 | Phase 306 | Done (b33e388e) |
| WSAUTH-04 | Phase 306 | Done (b33e388e) |
| AUDIT-01 | Phase 307 | Complete (d5f9b387) |
| AUDIT-02 | Phase 307 | Complete (d5f9b387) |
| AUDIT-03 | Phase 307 | Complete (d5f9b387) |
| AUDIT-04 | Phase 307 | Complete (d5f9b387) |
| RBAC-01 | Phase 308 | Done (pre-built) |
| RBAC-02 | Phase 308 | Done (pre-built) |
| RBAC-03 | Phase 308 | Done (pre-built) |
| RBAC-04 | Phase 308 | Done (pre-built) |
| SECAUDIT-01 | Phase 309 | Done |
| SECAUDIT-02 | Phase 309 | Done |
| SECAUDIT-03 | Phase 309 | Done |

## Future Requirements (deferred)

- Certificate rotation automation (manual renewal — venue CA is long-lived)
- Per-customer JWT (currently only staff/pod JWTs)
- Encrypted SQLite at rest (not justified at venue scale)

## Out of Scope

- External CA / Let's Encrypt for internal HTTP — self-signed venue CA is correct for LAN
- OAuth2 / OIDC — JWT with roles is sufficient
- Network segmentation / VLANs — infrastructure, not software
- WAF / rate limiting on internal endpoints — venue LAN only
