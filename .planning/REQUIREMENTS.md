# Requirements: v38.0 Security Hardening & Operational Maturity

**Defined:** 2026-04-01
**Core Value:** Harden the attack surface after all data flows are established

## TLS for Internal HTTP (TLS)

- [ ] **TLS-01**: Self-signed venue CA generated via `scripts/generate-venue-ca.sh` with server + per-pod client certificates
- [ ] **TLS-02**: Axum server (:8080) accepts mTLS connections — rejects clients without valid venue CA cert
- [ ] **TLS-03**: rc-agent (:8090) accepts mTLS connections from server — rejects unauthorized callers
- [ ] **TLS-04**: Tailscale remote connections bypass mTLS (already encrypted) — mTLS is LAN-only

## WS Auth Hardening (WSAUTH)

- [ ] **WSAUTH-01**: Per-pod JWT tokens with 24-hour expiry replace static PSK for WebSocket authentication
- [ ] **WSAUTH-02**: JWT tokens auto-rotate 1 hour before expiry — zero-downtime refresh
- [ ] **WSAUTH-03**: Invalid or expired JWT on WebSocket causes immediate disconnect + WhatsApp alert to staff
- [ ] **WSAUTH-04**: PSK remains as bootstrap fallback — initial connection uses PSK, server issues JWT for subsequent auth

## Audit Log Integrity (AUDIT)

- [x] **AUDIT-01**: Every activity_log entry includes a SHA-256 hash linking to the previous entry (append-only chain)
- [x] **AUDIT-02**: Tamper detection: if any entry's previous_hash doesn't match the actual previous entry's hash, alert fires
- [x] **AUDIT-03**: Hash chain covers config changes, deploys, billing events, and admin actions
- [x] **AUDIT-04**: `GET /api/v1/audit/verify` endpoint returns chain integrity status (valid/broken + first broken entry)

## Role-Based Access Control (RBAC)

- [ ] **RBAC-01**: Three roles defined: cashier (billing only), manager (billing + config), superadmin (everything)
- [ ] **RBAC-02**: JWT tokens include role claim — server extracts and enforces on every protected endpoint
- [ ] **RBAC-03**: Cashier role can only access billing, customer, and cafe endpoints — config/deploy/admin returns 403
- [ ] **RBAC-04**: Admin dashboard UI shows/hides sections based on role — but enforcement is server-side (UI is convenience)

## Security Audit Script (SECAUDIT)

- [ ] **SECAUDIT-01**: `scripts/security-audit.sh` scans: open ports, TLS config, JWT validity, default credentials, chain integrity
- [ ] **SECAUDIT-02**: Output is structured JSON scorecard with pass/fail per check and overall score
- [ ] **SECAUDIT-03**: Script integrates with existing `gate-check.sh` as a pre-deploy security gate

## Traceability

| REQ | Phase | Status |
|-----|-------|--------|
| TLS-01 | Phase 305 | Pending |
| TLS-02 | Phase 305 | Pending |
| TLS-03 | Phase 305 | Pending |
| TLS-04 | Phase 305 | Pending |
| WSAUTH-01 | Phase 306 | Pending |
| WSAUTH-02 | Phase 306 | Pending |
| WSAUTH-03 | Phase 306 | Pending |
| WSAUTH-04 | Phase 306 | Pending |
| AUDIT-01 | Phase 307 | Complete (d5f9b387) |
| AUDIT-02 | Phase 307 | Complete (d5f9b387) |
| AUDIT-03 | Phase 307 | Complete (d5f9b387) |
| AUDIT-04 | Phase 307 | Complete (d5f9b387) |
| RBAC-01 | Phase 308 | Pending |
| RBAC-02 | Phase 308 | Pending |
| RBAC-03 | Phase 308 | Pending |
| RBAC-04 | Phase 308 | Pending |
| SECAUDIT-01 | Phase 309 | Pending |
| SECAUDIT-02 | Phase 309 | Pending |
| SECAUDIT-03 | Phase 309 | Pending |

## Future Requirements (deferred)

- Certificate rotation automation (manual renewal — venue CA is long-lived)
- Per-customer JWT (currently only staff/pod JWTs)
- Encrypted SQLite at rest (not justified at venue scale)

## Out of Scope

- External CA / Let's Encrypt for internal HTTP — self-signed venue CA is correct for LAN
- OAuth2 / OIDC — JWT with roles is sufficient
- Network segmentation / VLANs — infrastructure, not software
- WAF / rate limiting on internal endpoints — venue LAN only
