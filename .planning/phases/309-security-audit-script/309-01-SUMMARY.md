---
phase: 309
plan: "01"
subsystem: security-audit
tags: [security, audit, scorecard, gate-check, tooling]
dependency_graph:
  requires: [TLS-01, WSAUTH-01, AUDIT-01, RBAC-01]
  provides: [SECAUDIT-01, SECAUDIT-02, SECAUDIT-03]
  affects: [scripts, test]
tech_stack:
  added: []
  patterns: [bash+python JSON, temp file accumulation, gate-check integration]
key_files:
  created:
    - scripts/security-audit.sh
  modified:
    - test/gate-check.sh (Suite 7 added)
decisions:
  - "28 checks across 5 categories — covers all v38.0 phases"
  - "Live chain integrity check (3.4) gracefully skips if server unreachable"
  - "Temp file for JSON avoids shell escaping issues with complex error messages"
  - "Suite 7 in gate-check.sh — deploy blocked if overall=fail"
metrics:
  duration: "~20 minutes"
  completed: "2026-04-02"
  tasks_completed: 2
  tasks_total: 2
  files_modified: 1
  files_created: 1
---

# Phase 309 Plan 01: Security Audit Script Summary

Single-command security scorecard covering all v38.0 hardening — integrated into the deploy gate.

## What Was Built

### SECAUDIT-01: Comprehensive Security Scan
`scripts/security-audit.sh` — 28 checks across 5 categories:

**TLS (Phase 305) — 5 checks:**
1. CA generation script exists and executable
2. Server mTLS module with WebPkiClientVerifier
3. Agent TLS module present
4. Tailscale CGNAT bypass detection
5. TLS disabled by default (safe rollout)

**WS Auth (Phase 306) — 7 checks:**
1. PodClaims JWT struct defined
2. create_pod_jwt function present
3. Pod JWT decode with dual-secret rotation
4. authenticate_agent_ws (JWT→PSK fallback)
5. WhatsApp alert on JWT rejection
6. IssueJwt/RefreshJwt protocol messages
7. Agent JWT storage in AppState

**Audit Chain (Phase 307) — 4 checks:**
1. SHA-256 hash chain in activity_log
2. entry_hash/previous_hash DB columns
3. Audit verify endpoint defined
4. Live chain integrity (if server reachable)

**RBAC (Phase 308) — 6 checks:**
1. Three role constants defined
2. StaffClaims.role field in JWT
3. Manager+ routes via require_role_manager
4. Superadmin routes via require_role_superadmin
5. Config/deploy/flags behind superadmin
6. staff_members.role DB column

**General — 7 checks:**
1. No .unwrap() in security-critical files
2. Pre-commit security hook installed
3. No hardcoded secrets in Rust source
4. JWT no default/weak secret
5. Static CRT configured
6. SEC-GATE-01 security-check.js present
7. Pod endpoints behind service key auth

### SECAUDIT-02: JSON Scorecard
Output: `security-scorecard.json` with:
```json
{
  "version": "v38.0",
  "timestamp": "...",
  "checks": [{name, status, details, category}, ...],
  "summary": {total, pass, fail, warn},
  "score": "25/28",
  "overall": "pass"
}
```

### SECAUDIT-03: gate-check.sh Integration
Suite 7 added to `test/gate-check.sh`:
- Runs `scripts/security-audit.sh` during pre-deploy
- Extracts score summary
- Deploy blocked if `overall=fail`

## Test Results

First run: **25/28 PASS, 0 FAIL, 3 WARN** — overall **PASS**.
