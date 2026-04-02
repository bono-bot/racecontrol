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

### Progress Table

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 305. TLS for Internal HTTP | 1/1 | Complete ✅ | 2026-04-01 |
| 306. WS Auth Hardening | 1/1 | Complete ✅ | b33e388e |
| 307. Audit Log Integrity | 1/1 | Complete | d5f9b387 |
| 308. RBAC for Admin | 1/1 | Complete ✅ (pre-built) | 2026-04-02 |
| 309. Security Audit Script | 1/1 | Complete ✅ | 2026-04-02 |

---

## Phase Details

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

*Last updated: 2026-04-01 after v38.0 milestone initialization*
